# 持久化与恢复

## 1. 持久化原则

数据库是 Task、Phase、ExecutionPlan、ToolCall 和 State 的唯一事实来源。

关键原则：

- Task 必须先持久化再进入队列。
- 每个 Phase 的关键输出必须先持久化再推进状态。
- 已持久化输出是恢复时的完成凭证。
- 内存中的 Task、State 和队列项都可以丢失并从数据库重建。
- 外部工具副作用不能依赖数据库事务保证原子性，必须使用幂等和恢复策略。

下面的数据模型是逻辑模型，不限定具体数据库、ORM 和字段存储格式。

本文字段属于目标持久化模型；当前 Rust 代码只提供可序列化的领域类型和内存原型，尚未实现数据库、Lease、审计时间等持久化设施。

## 2. Task

建议字段：

| 字段 | 用途 |
| --- | --- |
| `id` | Task 唯一标识。 |
| `phase` | 当前持久化 Phase。 |
| `scheduling_status` | Ready、Queued、Running、Suspended 或 Terminal。 |
| `state` | 当前 State 的结构化内容。 |
| `state_version` | State 乐观锁版本。 |
| `phase_version` | 识别重复队列消息和并发推进。 |
| `lease_owner` | 当前认领者。 |
| `lease_expires_at` | Lease 过期时间。 |
| `next_run_at_ms` | 下次允许调度时间，使用 Unix 毫秒时间戳；未设置表示可以立即调度。 |
| `created_at`、`updated_at` | 审计时间。 |

`phase_version` 在每次 Phase 成功迁移时单调递增。`state_version` 只在 State 成功提交时递增。

## 3. ExecutionPlan

建议字段：

| 字段 | 用途 |
| --- | --- |
| `id` | ExecutionPlan 唯一标识。 |
| `task_id` | 所属 Task。 |
| `ordinal` | Task 内的 ExecutionPlan 顺序编号，通常从 `0` 开始。 |
| `state_version` | 决策时使用的 State 版本。 |
| `tool_calls` | 该 ExecutionPlan 中的 ToolCall 集合。 |
| `status` | Planned、Ready、Running、Succeeded、Failed 或 Conflict。 |
| `created_at`、`completed_at` | 审计时间。 |

ExecutionPlan 必须记录决策时使用的 State 版本。参数生成和工具执行发现 State 已被其他流程修改时，不得静默继续使用过期快照。

`Decision` 不作为 `ExecutionPlan` 的字段。若需要审计 LLM 原始输出，应由独立的 `DecisionRecord` 保存，并关联它生成的 `ExecutionPlan`；当前 Rust 模型已定义 `Decision`，但尚未实现 `DecisionRecord`。

## 4. ToolCall

建议字段：

| 字段 | 用途 |
| --- | --- |
| `id` | ToolCall 唯一标识。 |
| `execution_plan_id` | 所属 ExecutionPlan。 |
| `plan.call_key` | ExecutionPlan 内稳定逻辑标识。 |
| `plan.tool_name` | 工具名称。 |
| `plan.purpose` | 决策阶段描述的调用目的。 |
| `execution.argument_revision` | 参数修订版本。 |
| `execution.arguments` | 已校验的结构化参数。 |
| `execution.status` | 参数和执行状态。 |
| `execution.attempt` | 已执行次数。 |
| `execution.next_retry_at_ms` | 下次允许重试时间。 |
| `execution.idempotency_key` | 写工具幂等键。 |
| `execution.result` | 结构化工具结果。 |
| `execution.result.state_delta` | 工具建议的 State 变化。 |
| `execution.error_records` | 工具调用的错误记录集合。 |

建议建立以下唯一约束：

- `(execution_plan_id, plan.call_key)` 唯一。
- `(plan.tool_name, execution.idempotency_key)` 在需要幂等的工具调用范围内唯一。
- 同一个 ToolCall 同一时间只有一个有效参数 revision。

ToolCall 的逻辑状态包括：参数待生成、参数有效、等待审批、待执行、执行中、等待重试、成功、失败、执行结果未知和已失效。

## 5. FinalResponse

最终回答应单独持久化或作为 Task 的不可变完成字段持久化，至少保存：

- `task_id`
- `state_version`
- LLM 原始输出
- 对用户展示的最终内容
- 生成时间

FinalResponse 持久化和 Task 进入 `Completed` 必须在同一事务中完成。

## 6. State

State 至少包含：

- 用户目标和有效输入。
- 当前任务已经确认的事实。
- 当前任务需要的业务知识。
- 已执行 ExecutionPlan 和 ToolCall 的结构化结果。
- 失败、重试耗尽和无法确定的执行事实。
- 当前业务目标的完成进度。
- 等待用户补充或审批的信息。

State 可以作为结构化快照保存，同时保留 ExecutionPlan、ToolCall 和 StateDelta 作为审计与重建依据。当前代码使用 `serde_json::Value` 保存 State 数据；目标架构是否引入强类型领域结构仍见[实施计划中的待决策项](implementation-plan.md#12-实现前待决策项)。

## 7. 事务边界

以下操作必须具有原子性：

1. 创建 Task 并设置初始 Phase。
2. 持久化决策结果、创建 ExecutionPlan 和 ToolCall、推进到参数 Phase。
3. 持久化单个 ToolCall 参数及其 revision。
4. 持久化单个 ToolCall 的一次执行结果或错误及 attempt。
5. 合并 ExecutionPlan 结果、更新 State、更新 ExecutionPlan 状态、推进 Task Phase。
6. 持久化最终回答并将 Task 标记为 Completed。

外部工具调用不能与本地数据库事务组成普遍可靠的原子事务，因此不包含在上述事务中。

## 8. 并发控制

### 8.1 Phase CAS

Worker 使用 `task_id`、`expected_phase` 和 `expected_version` 原子认领 Phase。更新影响行数为零表示任务已经被其他 Worker 推进，当前 job 返回 `Stale`。

### 8.2 State 乐观锁

StateReducer 提交时必须验证 ExecutionPlan 使用的 `state_version`。版本不匹配时不得覆盖新 State，应记录冲突并重新决策。

### 8.3 ToolCall 结果提交

ToolCall 结果提交必须验证调用状态、attempt 和参数 revision。迟到的旧 attempt 不能覆盖新 attempt 或新参数 revision 的结果。

## 9. 恢复流程

```mermaid
flowchart TD
    Scan[Dispatcher 扫描可恢复 Task] --> Lease{Lease 是否有效?}
    Lease -- 是 --> Skip[本轮跳过]
    Lease -- 否 --> Claim[认领 Task Phase]
    Claim --> Load[加载持久化 Phase 输出]
    Load --> Completed{当前 Phase 输出是否已完成?}
    Completed -- 是 --> Advance[只推进或重新调度]
    Completed -- 否 --> Resume[恢复未完成工作]
    Resume --> Persist[持久化结果]
    Advance --> Persist
```

恢复永远从数据库中的 Phase、version 和阶段输出开始，不根据旧进程内存推断进度。

## 10. 崩溃恢复场景

### 10.1 Phase 完成后未重新入队

Phase 结果已经持久化，但进程在发送唤醒信号前崩溃。Dispatcher 的周期扫描会发现 Task 处于 `Ready`，并重新入队。

### 10.2 LLM 返回后未持久化

Lease 过期后重新执行当前 Phase。若模型服务不支持幂等，可能产生额外调用费用；未持久化的输出不能作为可信恢复依据。

### 10.3 部分 ToolCall 已完成

已完成结果单独持久化。恢复执行 Phase 时只执行未完成、重试到期或 Lease 过期的 ToolCall。

### 10.4 写工具成功后未持久化

使用同一幂等键恢复调用。若外部系统不支持幂等和结果查询，则标记为 `ExecutionUnknown` 并停止自动执行。

### 10.5 ExecutionPlan 已归并但 Phase 未推进

State 提交、ExecutionPlan 状态更新和 Task Phase 迁移属于同一个事务，因此不能只完成其中一部分。事务结果未知时通过 Task 和 ExecutionPlan 的版本重新读取，不重复应用 StateDelta。

## 11. 数据保留与审计

- LLM 原始输出和解析结果应同时保存，便于审计 Schema 兼容问题。
- ToolCall 的每次 attempt 应有可追踪记录，不能只保留最后一次错误。
- 参数 revision、审批记录和幂等键必须关联。
- State 快照可以压缩或归档，但不能破坏仍在运行 Task 的恢复能力。
- 删除或脱敏历史数据时必须遵守业务数据保留策略。

## 12. 持久化不变量

1. Phase 输出与 Phase 迁移原子提交。
2. 同一 StateDelta 最多应用一次。
3. State 和 Phase version 单调递增。
4. 迟到结果不能覆盖新参数 revision。
5. 已完成 ToolCall 结果不可被自动清空或回退。
6. 最终回答存在时 Task 必须处于 Completed，反之亦然。

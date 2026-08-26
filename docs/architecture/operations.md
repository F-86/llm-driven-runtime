# 运行保障

## 1. 资源控制

系统至少需要以下限制：

- ReadyTaskQueue 最大容量。
- Worker 最大并发 Task 数。
- 单个 ExecutionPlan 最大 ToolCall 数。
- 全局工具调用并发数。
- 单个工具的并发数和速率限制。
- 单个 Task 最大 ExecutionPlan 数。
- 单个 Task 最大工具执行次数。
- LLM token、费用和请求次数上限。
- 单个 Phase 和整个 Task 的超时时间。
- 重复决策和重复调用检测。

异步 I/O、CPU 密集操作和阻塞 SDK 应使用不同的执行资源，避免互相拖垮。

资源限制分为软限制和硬限制：

- 软限制允许 Runtime 延后执行或降低并发。
- 硬限制达到后必须停止自动推进，并由决策生成明确结果或将 Task 置为失败。

## 2. 失败语义

| 层级 | 失败含义 | 后续处理 |
| --- | --- | --- |
| LLM 调用失败 | 当前 Phase 暂时无法得到输出。 | 按 LLM 策略重试或延后 Phase。 |
| 参数生成失败 | 当前 ToolCall 无法得到有效参数。 | 重新生成无效调用、请求用户输入或回到决策。 |
| ToolCall 失败 | 单个工具当前参数执行失败。 | 根据工具策略重试。 |
| ExecutionPlan 失败 | 至少一个 ToolCall 重试耗尽、`ExecutionUnknown` 或 Delta 冲突。 | 保留全部事实并进入下一次决策。 |
| Task 失败 | 决策明确判断无法继续，或超过硬性资源上限。 | 持久化最终错误并进入终态。 |
| Runtime 系统失败 | 数据库、调度器或内部不变量异常。 | 不把系统错误伪装成业务失败，保留任务用于恢复。 |

系统错误、业务错误和用户输入不足必须使用不同错误类型，不能只依赖错误字符串判断。

## 3. 超时与取消

- ToolCall 超时由工具配置和全局上限共同决定。
- Phase 超时不代表外部写操作一定被取消，恢复时仍需使用相同幂等键。
- 客户端断开不能取消后台 Worker 中的 Task。
- 任务主动取消需要持久化取消意图，并根据工具能力决定停止、等待或标记结果未知。
- 优雅停机停止新认领，并让运行中的短 Phase 在限定时间内完成。

## 4. 可观测性与审计

每次 Phase 至少记录：

- `task_id`
- `phase`
- `phase_version`
- `execution_plan_id`
- `tool_call_id`
- `attempt`
- `worker_id`
- `lease_owner`
- 开始时间、结束时间和耗时
- LLM token 和费用
- 工具结果分类
- State 版本变化

建议使用贯穿 Task、ExecutionPlan 和 ToolCall 的 trace（追踪）上下文。日志中不得直接输出密钥、访问令牌和未脱敏敏感参数。

关键指标包括：

- 可运行任务数量和最老任务等待时间。
- 队列使用率和 Dispatcher 拉取数量。
- 各 Phase 成功率和耗时。
- 工具成功率、重试率和重试耗尽率。
- LLM 调用次数、失败率、token 和费用。
- Lease 过期恢复次数。
- StateDelta 冲突次数。
- 等待用户输入和审批的任务数量。

## 5. 安全与权限

- 工具筛选和执行前都必须校验权限，不能只依赖 LLM 提示词。
- ToolCall 参数持久化前应执行敏感字段处理。
- 写工具默认要求更严格的权限、幂等和审计策略。
- 工具只能产生其元数据允许范围内的 StateMutation。
- 用户补参和审批操作必须校验 Task 所属关系和参数 revision。
- LLM 输出始终被视为不可信输入，必须经过结构和业务校验。
- 外部工具凭证由执行环境注入，不写入 State、Prompt 或 ToolCall 参数历史。

## 6. 循环与重复检测

虽然单次 `handle` 不包含整个任务循环，但 Task 仍可能在多个 Phase 之间长期循环。Runtime 应检测：

- 连续产生相同 ExecutionPlan 决策。
- 使用相同参数重复创建逻辑等价 ToolCall。
- 工具失败后未改变参数或策略就再次调用。
- `NeedSummary` 与 `NeedDecision` 反复切换。
- Task 已经超过 ExecutionPlan、工具调用、token、费用或时间预算。

检测到循环后，将事实写入 State 并允许一次受限决策；仍无法退出时进入 Task 失败终态。

## 7. 运行保障不变量

1. 系统错误不得伪装成业务失败。
2. 达到硬限制后不得继续自动产生工具副作用。
3. 敏感数据不得出现在未脱敏日志和 Prompt 中。
4. 监控指标必须能区分 Task、ExecutionPlan 和 ToolCall 失败。
5. Task 终态必须包含可供用户或运维理解的原因。

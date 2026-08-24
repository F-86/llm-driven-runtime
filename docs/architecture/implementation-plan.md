# 实施计划

## 1. 计划原则

实施过程先验证领域语义和状态机，再引入并行、数据库和真实 LLM。每个里程碑都必须保持已有测试可运行，并为新增不变量补充测试。

本文档只描述实施顺序和决策点，不授权直接修改代码。每个里程碑开始前应由项目维护者确认范围和具体技术方案。

## 2. 从当前实现迁移

当前实现可以按以下顺序演进，避免一次性重写全部组件：

1. 将单工具选择结果扩展为 `Decision`，支持 `ExecuteStage`、`Finish`、`NeedUserInput` 和 `Abort`。
2. 引入 Task、Phase、Stage 和 ToolCall 的领域类型，但先使用内存 Repository 验证状态机。
3. 将当前 `handle` 拆成 Phase Dispatcher 和独立 Handler。
4. 将工具参数从字符串逐步改为结构化 `serde_json::Value`，保持 Schema 校验。
5. 将 `ToolResult` 扩展为结构化结果和 StateDelta。
6. 实现有界并行 Execution Handler、结果持久化和统一 StateReducer。
7. 引入数据库 Repository、事务、版本和 Lease。
8. 引入有界任务队列、Dispatcher 和周期恢复。
9. 增加幂等、重试、资源限制和可观测性。
10. 最后补充人工补参和审批恢复接口。

```mermaid
flowchart LR
    M0[冻结核心契约] --> M1[内存状态机]
    M1 --> M2[Stage 并行与 StateDelta]
    M2 --> M3[持久化与恢复]
    M3 --> M4[队列与 Dispatcher]
    M4 --> M5[重试、幂等与限制]
    M5 --> M6[真实 LLM 与完整回答]
    M6 --> M7[人工参与占位与扩展]
```

## 3. 里程碑 0：冻结核心契约

目标是先确定领域语义，不实现运行逻辑。

交付物：

- `TaskPhase`、`SchedulingStatus`、`Decision`、`Stage`、`ToolCall` 和 `HandleOutcome` 的最终定义。
- Tool 成功、失败、重试和 StateDelta 接口。
- Phase 状态迁移表和非法迁移规则。
- 第一版范围和默认资源限制。

验收标准：

- 所有 Phase 都有明确输入、输出、持久化结果和下一状态。
- 不存在由多个组件同时负责推进 Task Phase 的情况。
- 领域类型不依赖具体数据库、队列或 LLM SDK。

决策门：

- 确认 State 和 StateMutation 的表示。
- 确认参数生成调用粒度。
- 确认第一版人工参与范围。

## 4. 里程碑 1：内存状态机

目标是在不引入数据库和真实 LLM 的情况下验证 Phase 模型。

交付物：

- Repository trait 和内存实现。
- Phase Dispatcher。
- 固定输出的 Decision Handler、Argument Handler 和 Summary Handler。
- 单工具 Execution Handler。
- 状态迁移和重复 RuntimeJob 测试。

验收标准：

- 一次 `handle` 只推进一个 Phase。
- 重复提交同一个 `RuntimeJob` 不会重复推进。
- 已持久化的阶段输出会被复用。
- 非法 Phase 迁移被确定性拒绝。

## 5. 里程碑 2：Stage 并行和 StateDelta

目标是实现当前设计的核心价值。

交付物：

- 多 ToolCall Stage。
- 有界异步并行执行。
- 每个 ToolCall 独立结果记录。
- StateReducer 和稳定合并顺序。
- 部分成功、重试耗尽和 Delta 冲突测试。

验收标准：

- 同一 Stage 的独立工具能够并行执行。
- ToolCall 不直接修改 State。
- 部分失败时保留成功结果。
- Stage 结束后只提交一次 State。
- 不同并发完成顺序得到相同 State。

决策门：

- 确认第一版并行安全校验范围。
- 确认 Tool Executor 默认并发数。

## 6. 里程碑 3：持久化与崩溃恢复

目标是让 Task 在进程退出后可以恢复。

交付物：

- 数据库 Repository 和 Schema migration（迁移）。
- Phase CAS、State version 和事务边界。
- Lease、心跳和过期认领。
- 已完成 ToolCall 跳过和恢复测试。

验收标准：

- Phase 任意持久化边界后重启都能继续。
- 重复队列消息不会导致已完成内容重新生成或执行。
- 多 Worker 不能同时提交同一 Phase。
- 迟到的 ToolCall attempt 不能覆盖新结果。

决策门：

- 确认数据库和 migration 工具。
- 确认 Lease 时长和心跳间隔。
- 确认 Task、State 和执行历史的数据保留策略。

## 7. 里程碑 4：队列与 Dispatcher

目标是实现有界调度和背压。

交付物：

- 有界 ReadyTaskQueue。
- 数据库优先的任务提交服务。
- 唤醒信号和每秒兜底扫描。
- 按队列剩余容量认领任务。
- Worker Pool 和优雅停机。

验收标准：

- 队列满时任务仍然安全保存在数据库。
- 进程重启后未完成任务自动恢复。
- 多实例不会长期重复消费同一个 Task Phase。
- 通知丢失时任务仍能由周期扫描运行。

决策门：

- 确认 ReadyTaskQueue 和 Worker 默认容量。
- 确认单实例与多实例第一版范围。

## 8. 里程碑 5：重试、幂等和执行限制

目标是补齐生产运行所需的执行语义。

交付物：

- 工具级 RetryPolicy。
- attempt、next_retry_at 和错误分类。
- 稳定幂等键和 ExecutionUnknown。
- Task、Stage、工具、时间、token 和费用限制。
- 重复 Stage 和重复 ToolCall 检测。

验收标准：

- 可重试错误不会超过工具配置的次数。
- 确定性错误不会使用相同参数机械重试。
- 写工具恢复时使用相同幂等键。
- 达到硬限制时 Task 能够确定性结束。

决策门：

- 确认默认重试退避策略。
- 确认高风险写工具进入 ExecutionUnknown 后的处理方式。
- 确认第一版资源预算默认值。

## 9. 里程碑 6：真实 LLM 与完整回答

目标是接入真实模型并完成端到端任务。

交付物：

- 结构化 Decision、Arguments 和 Summary LLM Adapter。
- Prompt 版本和输出 Schema 版本管理。
- 校验反馈和有限纠错。
- 最终回答持久化。
- token、费用和请求追踪。

验收标准：

- LLM 已持久化输出不会在恢复时重新生成。
- LLM 非法输出不会绕过 Runtime 校验。
- 最终回答只引用 State 中已经存在的事实。
- 参数纠错不会重新生成其他已经有效的参数。

决策门：

- 确认 LLM Provider 和结构化输出接口。
- 确认参数校验失败允许的纠错次数。
- 确认 Prompt 和 Schema 版本兼容策略。

## 10. 里程碑 7：人工参与占位与扩展

目标是保证人工流程不会破坏现有状态机。

交付物：

- `WaitingUserInput` 和 `WaitingApproval` 的持久化。
- 参数 revision 和审批失效规则。
- 第一版明确的“不支持继续交互”响应。
- 后续补参、审批和拒绝接口设计。

验收标准：

- 等待人工操作的 Task 不会被 Dispatcher 自动执行。
- 审批始终绑定具体参数 revision。
- 参数修改不会执行旧审批对应的 ToolCall。

决策门：

- 确认第一版只保存暂停状态，还是同时提供最小恢复接口。

## 11. 测试策略

### 11.1 状态机测试

- 每种合法 Phase 迁移。
- 非法 Phase 迁移拒绝。
- 重复和过期 RuntimeJob 返回 `Stale`。
- Phase 输出持久化后不重复调用 LLM。

### 11.2 并发测试

- 同一 Task 被多个 Worker 同时认领。
- Stage 中多个 ToolCall 并行完成顺序不同。
- StateDelta 使用稳定顺序合并。
- 队列容量达到上限时数据库任务不丢失。

### 11.3 恢复测试

- 每个事务提交前后模拟进程崩溃。
- LLM 输出持久化后崩溃。
- 部分 ToolCall 完成后崩溃。
- 写工具成功但本地结果未提交。
- Phase 完成但唤醒信号未发送。

### 11.4 属性测试

建议对以下不变量使用 property-based testing（属性测试）：

- 已完成 ToolCall 不会从成功状态回退。
- Task Phase version 单调递增。
- 同一 Stage 的 StateDelta 最多提交一次。
- 同一参数 revision 的幂等键保持稳定。
- Completed 和 Failed 状态不会被自动调度。

### 11.5 故障注入测试

- 在 LLM 返回前后中断 Phase。
- 在每个 ToolCall 完成前后中断 Worker。
- 在 StateReducer 提交前后中断进程。
- 模拟 Lease 过期、心跳延迟和重复恢复。
- 模拟外部工具超时但实际成功。

## 12. 实现前待决策项

以下事项不阻塞架构文档，但在对应里程碑开始前需要决定：

1. 第一版使用哪种数据库和迁移工具。
2. State 是强类型领域结构、通用 JSON，还是两者结合。
3. StateMutation 采用领域事件、JSON Patch，还是受限操作集合。
4. 参数生成是一次 LLM 调用生成整个 Stage，还是按 ToolCall 分别调用。
5. 参数校验失败时允许多少次 LLM 纠错。
6. Stage 并行安全第一版是否仅允许只读工具并行。
7. ReadyTaskQueue、Worker 和 Tool Executor 的默认容量。
8. Lease 时长、心跳间隔和默认重试退避。
9. 第一版 Repository 是先实现内存版本，还是直接接入数据库。
10. 人工参与第一版是只保存暂停状态，还是同时提供最小恢复接口。

这些决策应按里程碑逐步作出，不需要在开始实现前一次性全部确定。

## 13. 每个里程碑的执行方式

每个里程碑开始前：

1. 确认该里程碑的决策门。
2. 将接口和行为拆成可独立验收的小任务。
3. 先编写状态机和失败路径测试。
4. 实现最小闭环。
5. 运行当前工作区全部测试。
6. 对照[架构不变量](README.md#8-架构不变量)复核。
7. 更新架构文档中已经发生变化的契约。


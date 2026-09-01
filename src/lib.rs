//! LLM 驱动的任务运行时领域模型和单工具原型。

/// 当前单工具运行时原型。
pub mod runtime;
/// 任务调度相关类型。
pub mod scheduling;
/// 任务状态和状态变更类型。
pub mod state;
/// `Task`、`Phase`、`ExecutionPlan` 和 `ToolCall` 领域类型。
pub mod task;
/// 工具契约、注册表、选择器和参数生成器接口。
pub mod tool;
/// 当前原型接收的用户输入类型。
pub mod user_input;

/// 工具参数生成器。
pub mod argument_generator;
/// 工具契约和结果类型。
pub mod definition;
/// 内置工具实现。
pub mod implementations;
/// 工具注册表。
pub mod registry;
/// 工具选择器。
pub mod selector;

pub use definition::{
    ResourcePattern, RetryDecision, SideEffect, Tool, ToolError, ToolErrorKind, ToolMetadata,
    ToolRetryPolicy, ToolSuccess,
};
pub use implementations::*;
pub use selector::{SelectionError, ToolSelection, ToolSelector};

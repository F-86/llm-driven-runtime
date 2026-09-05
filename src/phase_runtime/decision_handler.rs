use std::fmt;

use crate::task::{Decision, Task};

/// 决策阶段生成 `Decision` 的接口。
///
/// 后续接入真实决策来源时，可以实现这个 trait，而无需改变 `PhaseRuntime` 的编排逻辑。
pub trait DecisionHandler: Send + Sync {
    /// 根据当前任务生成下一步决策。
    ///
    /// # Errors
    ///
    /// 如果决策来源无法产生结果，则返回 [`DecisionHandlerError`]。
    fn decide(&self, task: &Task) -> Result<Decision, DecisionHandlerError>;
}

/// 决策处理器返回的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionHandlerError {
    /// 便于诊断的错误信息。
    pub message: String,
}

impl DecisionHandlerError {
    /// 使用错误信息创建一个决策处理错误。
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DecisionHandlerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "决策处理失败：{}", self.message)
    }
}

impl std::error::Error for DecisionHandlerError {}

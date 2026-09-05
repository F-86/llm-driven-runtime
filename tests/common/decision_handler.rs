use llm_driven_runtime::{
    phase_runtime::{DecisionHandler, DecisionHandlerError},
    task::{Decision, Task},
};

/// 始终返回同一个决策的测试替身。
#[derive(Debug, Clone, PartialEq)]
pub struct FixedDecisionHandler {
    decision: Decision,
}

impl FixedDecisionHandler {
    /// 创建一个始终返回给定决策的处理器。
    #[must_use]
    pub fn new(decision: Decision) -> Self {
        Self { decision }
    }
}

impl DecisionHandler for FixedDecisionHandler {
    fn decide(&self, _task: &Task) -> Result<Decision, DecisionHandlerError> {
        Ok(self.decision.clone())
    }
}

/// 任务阶段枚举，用于状态机，表示“下一次 Runtime::handle 应该做什么”
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPhase {
    /// 等待 LLM 决定下一步
    NeedDecision,

    /// Stage 已确定，等待生成 ToolCall 参数
    NeedArguments,

    /// 参数已经准备完成，等待执行 Stage
    ReadyToExecute,

    /// 等待用户补全或修改参数
    WaitingUserInput,

    /// 等待用户审批写工具
    WaitingApproval,

    /// 任务目标已经满足，等待生成最终回答
    NeedSummary,

    /// 最终回答已经持久化
    Completed,

    /// Task 确定无法继续
    Failed,
}

impl TaskPhase {
    /// 判断是否允许迁移到目标 Phase
    pub fn can_transition_to(&self, next: &TaskPhase) -> bool {
        use TaskPhase::*;

        match self {
            NeedDecision => matches!(
                next,
                NeedArguments | NeedSummary | WaitingUserInput | Failed
            ),
            NeedArguments => matches!(
                next,
                ReadyToExecute | WaitingUserInput | WaitingApproval | NeedDecision
            ),
            ReadyToExecute => matches!(next, NeedDecision | NeedArguments),
            WaitingUserInput => matches!(next, NeedArguments | NeedDecision),
            WaitingApproval => matches!(next, ReadyToExecute | NeedArguments | NeedDecision),
            NeedSummary => matches!(next, Completed | NeedDecision),
            Completed | Failed => false,
        }
    }
}

#[cfg(test)]
mod tests {}

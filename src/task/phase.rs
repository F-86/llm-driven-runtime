use std::fmt;

/// 目标持久化运行时使用的任务 Phase 状态机。
///
/// 它表示下一次持久化 `Runtime::handle` 应该处理什么；当前单工具原型尚未使用此状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    /// 等待 LLM 决定下一步
    NeedDecision,

    /// 执行计划已确定，等待生成 `ToolCall` 参数
    NeedArguments,

    /// 参数已经准备完成，等待按照计划执行
    ReadyToExecute,

    /// 等待用户补充信息、补全或修改参数
    WaitingInput,

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
    #[must_use]
    pub fn can_transition_to(&self, next: &TaskPhase) -> bool {
        use TaskPhase::{
            Completed, Failed, NeedArguments, NeedDecision, NeedSummary, ReadyToExecute,
            WaitingApproval, WaitingInput,
        };

        match self {
            NeedDecision => matches!(next, NeedArguments | NeedSummary | WaitingInput | Failed),
            NeedArguments => matches!(
                next,
                ReadyToExecute | WaitingInput | WaitingApproval | NeedDecision
            ),
            ReadyToExecute => matches!(next, NeedDecision | NeedArguments),
            WaitingInput => matches!(next, NeedArguments | NeedDecision),
            WaitingApproval => matches!(next, ReadyToExecute | NeedArguments | NeedDecision),
            NeedSummary => matches!(next, Completed | NeedDecision),
            Completed | Failed => false,
        }
    }

    /// 任务是否终止
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// 任务是否等待用户操作
    #[must_use]
    pub fn is_suspended(self) -> bool {
        matches!(self, Self::WaitingInput | Self::WaitingApproval)
    }
}

/// 非法阶段转移
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPhaseTransition {
    /// 迁移前的 Phase
    pub from: TaskPhase,
    /// 迁移目标 Phase
    pub to: TaskPhase,
}

impl fmt::Display for InvalidPhaseTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "非法阶段转移：{:?} -> {:?}", self.from, self.to)
    }
}

impl std::error::Error for InvalidPhaseTransition {}

#[cfg(test)]
mod tests {
    use super::TaskPhase::{
        self, Completed, Failed, NeedArguments, NeedDecision, NeedSummary, ReadyToExecute,
        WaitingApproval, WaitingInput,
    };

    /// 所有 `TaskPhase`
    const ALL_PHASE: &[TaskPhase] = &[
        Completed,
        Failed,
        NeedArguments,
        NeedDecision,
        NeedSummary,
        ReadyToExecute,
        WaitingApproval,
        WaitingInput,
    ];

    /// 所有能转换的 `TaskPhase` 元组
    const ALL_TRANSITION_PHASE: &[(TaskPhase, TaskPhase)] = &[
        // 缺少用户输入
        (NeedDecision, WaitingInput),
        // 决策 NeedToolCall
        (NeedDecision, NeedArguments),
        // 决策 Finish
        (NeedDecision, NeedSummary),
        // 决策 Abort
        (NeedDecision, Failed),
        // 需要补全或修改
        (NeedArguments, WaitingInput),
        // 参数有效
        (NeedArguments, ReadyToExecute),
        // 写工具需要审批
        (NeedArguments, WaitingApproval),
        // 参数生成无法继续
        (NeedArguments, NeedDecision),
        // 用户提供输入
        (WaitingInput, NeedArguments),
        // 用户修改需求或放弃当前执行计划
        (WaitingInput, NeedDecision),
        // 审批通过
        (WaitingApproval, ReadyToExecute),
        // 参数被修改，需要重新校验
        (WaitingApproval, NeedArguments),
        // 拒绝审批或修改需求
        (WaitingApproval, NeedDecision),
        // 执行计划完成
        (ReadyToExecute, NeedDecision),
        // 参数被修改
        (ReadyToExecute, NeedArguments),
        // 最终回答已持久化
        (NeedSummary, Completed),
        // 总结发现仍缺少事实
        (NeedSummary, NeedDecision),
    ];

    /// 测试允许的转移和不允许的转移
    #[test]
    fn should_allow_valid_and_reject_invalid_transitions() {
        for from in ALL_PHASE {
            for to in ALL_PHASE {
                assert_eq!(
                    from.can_transition_to(to),
                    ALL_TRANSITION_PHASE.contains(&(*from, *to))
                );
            }
        }
    }
}

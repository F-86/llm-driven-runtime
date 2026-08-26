use crate::task::TaskPhase;

/// 调度状态枚举，表示“当前能不能调度”
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingStatus {
    /// 可以被 Dispatcher 调度
    Ready,

    /// 已经放入内存队列
    Queued,

    /// Worker 正在处理
    Running,

    /// 等待用户输入或审批，不允许自动调度
    Suspended,

    /// Task 已经结束
    Terminal,
}

impl From<TaskPhase> for SchedulingStatus {
    fn from(phase: TaskPhase) -> Self {
        match phase {
            TaskPhase::WaitingInput | TaskPhase::WaitingApproval => Self::Suspended,
            TaskPhase::Completed | TaskPhase::Failed => Self::Terminal,
            _ => Self::Ready,
        }
    }
}

impl SchedulingStatus {
    /// 任务是否可调度
    #[must_use]
    pub fn is_dispatchable(self) -> bool {
        matches!(self, Self::Ready)
    }

    /// 任务是否已终止
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal)
    }
}

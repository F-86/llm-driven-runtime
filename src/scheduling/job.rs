use crate::task::{TaskId, TaskPhase};

/// Runtime 队列任务
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeJob {
    /// 任务 id
    pub task_id: TaskId,
    /// 期待的阶段
    pub expected_phase: TaskPhase,
    /// 期待的阶段版本号
    pub expected_version: u64,
}

impl RuntimeJob {
    /// 创建一个带有 Phase 和版本校验条件的队列任务。
    #[must_use]
    pub fn new(task_id: TaskId, expected_phase: TaskPhase, expected_version: u64) -> Self {
        Self {
            task_id,
            expected_phase,
            expected_version,
        }
    }
}

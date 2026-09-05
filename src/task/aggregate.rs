use crate::{
    scheduling::{RuntimeJob, SchedulingStatus},
    state::State,
    task::{InvalidPhaseTransition, TaskId, TaskPhase},
};

/// 任务聚合及其当前运行状态。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    /// 任务 id
    pub id: TaskId,
    /// 用户目标
    pub user_goal: String,
    /// 任务阶段
    pub phase: TaskPhase,
    /// 调度状态
    pub scheduling_status: SchedulingStatus,
    /// 状态
    pub state: State,
    /// 状态的版本号，在 `state` 发生变化的时候会改变，用做乐观锁
    pub state_version: u64,
    /// 任务阶段的版本号，在 `phase` 发生变化的时候会改变，用做乐观锁
    pub phase_version: u64,
    /// 下一次允许调度任务的时间，使用 Unix 毫秒时间戳；`None` 表示可以立即调度
    pub next_run_at_ms: Option<u64>,
}

impl Task {
    /// 创建一个新任务，新任务处于 `NeedDecision` 阶段，调度状态是 `Ready`
    pub fn new(id: TaskId, user_goal: impl Into<String>, state: State) -> Self {
        Self {
            id,
            user_goal: user_goal.into(),
            phase: TaskPhase::NeedDecision,
            scheduling_status: SchedulingStatus::Ready,
            state,
            state_version: 0,
            phase_version: 0,
            next_run_at_ms: None,
        }
    }

    /// 转移到指定的阶段
    ///
    /// # Errors
    ///
    /// 如果状态转移失败，则会返回 `InvalidPhaseTransition` 错误
    pub fn transition_to(&mut self, next: TaskPhase) -> Result<(), InvalidPhaseTransition> {
        if !self.phase.can_transition_to(&next) {
            return Err(InvalidPhaseTransition {
                from: self.phase,
                to: next,
            });
        }

        self.phase = next;
        self.phase_version += 1;
        self.scheduling_status = next.into();

        Ok(())
    }

    /// 替换状态
    pub fn replace_state(&mut self, state: State) {
        self.state = state;
        self.state_version += 1;
    }

    /// 判断调度任务是否仍匹配当前 Task 的 Phase 和版本，避免处理过期或重复任务
    #[must_use]
    pub fn matches_job(&self, job: &RuntimeJob) -> bool {
        self.id == job.task_id
            && self.phase == job.expected_phase
            && self.phase_version == job.expected_version
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        scheduling::SchedulingStatus,
        state::State,
        task::{Task, TaskId, TaskPhase},
    };

    /// 创建一个新的 `Task`。
    fn new_task() -> Task {
        Task::new(TaskId::new("task-1"), "测试任务", State::default())
    }

    /// 验证 Phase 转移会同步更新 Phase 版本和调度状态。
    ///
    /// 方法：将新建 Task 转移到等待输入阶段，并检查三个受影响字段。
    #[test]
    fn should_update_phase_version_and_scheduling_status_when_transitioning() {
        let mut task = new_task();

        task.transition_to(TaskPhase::WaitingInput)
            .expect("Phase 转移应该成功");

        assert_eq!(task.phase, TaskPhase::WaitingInput);
        assert_eq!(task.phase_version, 1);
        assert_eq!(task.scheduling_status, SchedulingStatus::Suspended);
    }

    /// 验证替换 State 只增加 State 版本，不改变 Phase 版本。
    ///
    /// 方法：替换新建 Task 的 State，并比较两个版本及保存的数据。
    #[test]
    fn should_increment_only_state_version_when_replacing_state() {
        let mut task = new_task();

        let old_phase_version = task.phase_version;

        task.replace_state(State {
            data: serde_json::json!({"ready": true}),
        });

        assert_eq!(task.state_version, 1);
        assert_eq!(task.phase_version, old_phase_version);
        assert_eq!(task.state.data, serde_json::json!({"ready": true}));
    }
}

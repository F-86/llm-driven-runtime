use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    scheduling::RuntimeJob,
    task::{ExecutionPlan, ExecutionPlanId, InvalidPhaseTransition, Task, TaskId, TaskPhase},
};

/// Repository 操作失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    /// 尝试创建一个已经存在的任务。
    TaskAlreadyExists(TaskId),

    /// 任务不存在。
    TaskNotFound(TaskId),

    /// 执行计划已经存在。
    ExecutionPlanAlreadyExists(ExecutionPlanId),

    /// 执行计划所属的 Task 与目标 Task 不一致。
    PlanTaskMismatch {
        /// 期望 Task 的 id。
        expected_task_id: TaskId,
        /// 执行计划实际所属 Task 的 id。
        actual_task_id: TaskId,
        /// 执行计划 id。
        plan_id: ExecutionPlanId,
    },

    /// Task Phase 非法迁移。
    InvalidPhaseTransition(InvalidPhaseTransition),

    /// Repository 的互斥锁已经中毒。
    LockPoisoned,
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskAlreadyExists(task_id) => {
                write!(formatter, "Task 已存在：{task_id}")
            }
            Self::TaskNotFound(task_id) => {
                write!(formatter, "Task 不存在：{task_id}")
            }
            Self::ExecutionPlanAlreadyExists(execution_plan_id) => {
                write!(formatter, "ExecutionPlan 已存在：{execution_plan_id}")
            }
            Self::PlanTaskMismatch {
                expected_task_id,
                actual_task_id,
                plan_id,
            } => {
                write!(
                    formatter,
                    "ExecutionPlan {plan_id} 所属 Task 不匹配，期望：{expected_task_id}，实际：{actual_task_id}"
                )
            }
            Self::InvalidPhaseTransition(error) => {
                write!(formatter, "{error}")
            }
            Self::LockPoisoned => formatter.write_str("Repository 锁已经中毒，内部数据可能不一致"),
        }
    }
}

impl std::error::Error for RepositoryError {}

/// Repository 原子提交的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutcome {
    /// 成功保存执行计划并推进了 Task。
    Committed,

    /// `RuntimeJob` 已经过期或重复，没有发生任何写入。
    Stale,
}

/// 进程内的任务 Repository。
///
/// Repository 负责保存和读取任务数据。
/// 通过 `clone` 得到的多个 Repository 值共享同一份底层数据。
#[derive(Clone, Default)]
pub struct InMemoryRepository {
    /// 由 `Arc` 共享、由 `Mutex` 保护的内部数据。
    ///
    /// `Arc` 允许多个运行时共同访问 Repository；
    /// `Mutex` 保证同一时间只有一个线程修改数据。
    inner: Arc<Mutex<RepositoryInner>>,
}

/// Repository 实际保存的数据。
///
/// 这个结构体不对外公开，所有访问都应该通过 `InMemoryRepository` 提供的方法完成。
#[derive(Default)]
struct RepositoryInner {
    /// 按任务 id 保存当前任务。
    tasks: HashMap<TaskId, Task>,

    /// 按执行计划 id 保存执行计划。
    execution_plans: HashMap<ExecutionPlanId, ExecutionPlan>,
}

impl InMemoryRepository {
    /// 创建一个空的内存 Repository。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建 `Task`。
    ///
    /// # Errors
    ///
    /// 如果相同 id 的 `Task` 已经存在，则返回 [`RepositoryError::TaskAlreadyExists`]。
    pub fn create_task(&self, task: Task) -> Result<(), RepositoryError> {
        let task_id = task.id.clone();
        let mut inner = self.lock()?;

        match inner.tasks.entry(task_id.clone()) {
            Entry::Occupied(_) => Err(RepositoryError::TaskAlreadyExists(task_id)),
            Entry::Vacant(entry) => {
                entry.insert(task);
                Ok(())
            }
        }
    }

    /// 根据任务 id 读取任务。
    ///
    /// 返回任务的克隆值，而不是暴露 Repository 内部的引用。
    ///
    /// # Errors
    ///
    /// 如果 Repository 的互斥锁已经中毒，则返回 [`RepositoryError::LockPoisoned`]。
    pub fn get_task(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError> {
        let inner = self.lock()?;
        Ok(inner.tasks.get(task_id).cloned())
    }

    /// 根据执行计划 id 读取执行计划。
    ///
    /// 返回执行计划的克隆值，而不是暴露 Repository 内部的引用。
    ///
    /// # Errors
    ///
    /// 如果 Repository 的互斥锁已经中毒，则返回 [`RepositoryError::LockPoisoned`]。
    pub fn get_execution_plan(
        &self,
        execution_plan_id: &ExecutionPlanId,
    ) -> Result<Option<ExecutionPlan>, RepositoryError> {
        let inner = self.lock()?;
        Ok(inner.execution_plans.get(execution_plan_id).cloned())
    }

    /// 原子保存执行计划并推进 Task Phase。
    ///
    /// Task id、期待 Phase 和期待版本号必须同时匹配。
    /// 如果校验失败，Task 和 `ExecutionPlan` 都不会被修改。
    ///
    /// # Errors
    ///
    /// 如果 Repository 的互斥锁已经中毒，则返回 [`RepositoryError::LockPoisoned`]；
    /// 如果 Task 不存在，则返回 [`RepositoryError::TaskNotFound`]；
    /// 如果执行计划所属 Task 不匹配，则返回 [`RepositoryError::PlanTaskMismatch`]；
    /// 如果 Phase 迁移非法，则返回 [`RepositoryError::InvalidPhaseTransition`]；
    /// 如果执行计划已经存在，则返回 [`RepositoryError::ExecutionPlanAlreadyExists`]。
    pub fn commit_plan_and_transition(
        &self,
        job: &RuntimeJob,
        plan: ExecutionPlan,
        next_phase: TaskPhase,
    ) -> Result<CommitOutcome, RepositoryError> {
        let mut inner = self.lock()?;

        // 拆开两个 map，允许同时持有它们各自的 entry。
        let RepositoryInner {
            tasks,
            execution_plans,
        } = &mut *inner;

        // 查找 Task，并保留这个 entry，后面直接用它写回。
        let mut task_entry = match tasks.entry(job.task_id.clone()) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => {
                return Err(RepositoryError::TaskNotFound(job.task_id.clone()));
            }
        };

        // 复制一份，避免校验失败时修改 Repository 中的原始 Task。
        let current_task = task_entry.get().clone();
        if !current_task.matches_job(job) {
            return Ok(CommitOutcome::Stale);
        }

        if plan.task_id != current_task.id {
            return Err(RepositoryError::PlanTaskMismatch {
                expected_task_id: current_task.id.clone(),
                actual_task_id: plan.task_id.clone(),
                plan_id: plan.id.clone(),
            });
        }

        let mut updated_task = current_task;
        updated_task
            .transition_to(next_phase)
            .map_err(RepositoryError::InvalidPhaseTransition)?;

        let plan_id = plan.id.clone();
        match execution_plans.entry(plan_id.clone()) {
            Entry::Occupied(_) => Err(RepositoryError::ExecutionPlanAlreadyExists(plan_id)),
            Entry::Vacant(plan_entry) => {
                plan_entry.insert(plan);
                task_entry.insert(updated_task);

                Ok(CommitOutcome::Committed)
            }
        }
    }

    /// 获取内部数据的独占访问权。
    ///
    /// 如果之前持有锁的线程发生 `panic`，标准库会将锁标记为中毒。
    /// 本方法将这种情况转换成 `RepositoryError`，不继续使用可能已经不一致的数据。
    fn lock(&self) -> Result<MutexGuard<'_, RepositoryInner>, RepositoryError> {
        self.inner.lock().map_err(|_| RepositoryError::LockPoisoned)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        scheduling::{RuntimeJob, SchedulingStatus},
        state::State,
        task::{
            ExecutionPlan, ExecutionPlanId, ExecutionPlanStatus, Task, TaskId, TaskPhase, ToolCall,
            ToolCallId, ToolCallPlan,
        },
    };

    use super::{CommitOutcome, InMemoryRepository, RepositoryError};

    /// `create_task` 应该成功，并且调用 `get_task` 得到的 `Task` 应该和创建时传入的一样。
    #[test]
    fn should_create_and_read_task() {
        let repository = InMemoryRepository::new();
        let task = Task::new(TaskId::new("task-1"), "测试任务", State::default());

        repository
            .create_task(task.clone())
            .expect("创建 Task 应该成功");

        assert_eq!(
            repository.get_task(&task.id).expect("读取 Task 应该成功"),
            Some(task)
        );
    }

    /// `create_task` 应该拒绝重复 `Task`。
    #[test]
    fn should_reject_duplicate_task() {
        let repository = InMemoryRepository::new();
        let task = Task::new(TaskId::new("task-1"), "测试任务", State::default());

        repository.create_task(task.clone()).unwrap();

        assert_eq!(
            repository.create_task(task.clone()),
            Err(RepositoryError::TaskAlreadyExists(task.id))
        );
    }

    /// 持锁线程发生 panic 后，Repository 应该返回锁中毒错误。
    #[test]
    fn should_return_error_when_lock_is_poisoned() {
        let repository = InMemoryRepository::new();
        let repository_for_panic = repository.clone();

        let result = std::thread::spawn(move || {
            let _guard = repository_for_panic.inner.lock().unwrap();

            panic!("故意让 Repository 锁中毒");
        })
        .join();
        assert!(result.is_err());

        assert_eq!(
            repository.get_task(&TaskId::new("task-1")),
            Err(RepositoryError::LockPoisoned)
        );
    }

    /// 读取不存在的执行计划时应该返回 `None`。
    #[test]
    fn should_return_none_when_execution_plan_does_not_exist() {
        let repository = InMemoryRepository::new();

        assert_eq!(
            repository
                .get_execution_plan(&ExecutionPlanId::new("plan-1"))
                .expect("读取执行计划应该成功"),
            None
        );
    }

    /// 构建执行计划。
    fn build_plan(task_id: &TaskId) -> ExecutionPlan {
        let plan_id = ExecutionPlanId::new("plan-1");

        let tool_call = ToolCall::new(
            ToolCallId::new("call-1"),
            plan_id.clone(),
            ToolCallPlan {
                call_key: "call-1".to_string(),
                tool_name: "test-tool".to_string(),
                purpose: "测试调用".to_string(),
            },
        );

        ExecutionPlan::try_new(plan_id, task_id.clone(), 0, 0, vec![tool_call])
            .expect("测试计划应满足 ExecutionPlan 的条件")
    }

    /// 成功提交时应该同时保存执行计划并推进 Task。
    #[test]
    fn should_commit_plan_and_advance_task_atomically() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Ok(CommitOutcome::Committed)
        );

        let task = repository.get_task(&task_id).unwrap().unwrap();
        assert_eq!(task.phase, TaskPhase::NeedArguments);
        assert_eq!(task.phase_version, 1);
        assert_eq!(task.state_version, 0);
        assert_eq!(task.scheduling_status, SchedulingStatus::Ready);

        let plan = repository.get_execution_plan(&plan_id).unwrap().unwrap();
        assert_eq!(plan.status(), ExecutionPlanStatus::Planned);
        assert_eq!(plan.task_id, task_id);
        assert_eq!(plan.state_version, 0);
        assert_eq!(plan.tool_calls.len(), 1);
    }

    /// 非法 Phase 迁移时，不应该保存执行计划或修改 Task。
    #[test]
    fn should_not_partially_commit_when_transition_is_invalid() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        let result = repository.commit_plan_and_transition(&job, plan, TaskPhase::ReadyToExecute);

        assert!(matches!(
            result,
            Err(RepositoryError::InvalidPhaseTransition(_))
        ));

        let task = repository.get_task(&task_id).unwrap().unwrap();
        assert_eq!(task.phase, TaskPhase::NeedDecision);
        assert_eq!(task.phase_version, 0);

        assert_eq!(repository.get_execution_plan(&plan_id).unwrap(), None);
    }

    /// Job 版本过期时，不应该发生任何写入。
    #[test]
    fn should_return_stale_when_job_version_does_not_match() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let stale_job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 1);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&stale_job, plan, TaskPhase::NeedArguments),
            Ok(CommitOutcome::Stale)
        );

        let task = repository.get_task(&task_id).unwrap().unwrap();
        assert_eq!(task.phase, TaskPhase::NeedDecision);
        assert_eq!(task.phase_version, 0);
        assert_eq!(repository.get_execution_plan(&plan_id).unwrap(), None);
    }

    /// 同一个 Job 第二次提交时应该返回 `Stale`。
    #[test]
    fn should_not_commit_same_job_twice() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan.clone(), TaskPhase::NeedArguments,),
            Ok(CommitOutcome::Committed)
        );

        let saved_plan = repository.get_execution_plan(&plan_id).unwrap();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments,),
            Ok(CommitOutcome::Stale)
        );

        assert_eq!(repository.get_execution_plan(&plan_id).unwrap(), saved_plan);
    }

    /// Task 不存在时不应该保存执行计划。
    #[test]
    fn should_return_task_not_found() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("missing-task");
        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task_id);

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Err(RepositoryError::TaskNotFound(task_id))
        );
    }

    /// 执行计划属于其他 Task 时，不应该发生任何写入。
    #[test]
    fn should_reject_plan_for_another_task() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");
        let another_task_id = TaskId::new("task-2");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&another_task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Err(RepositoryError::PlanTaskMismatch {
                expected_task_id: task_id.clone(),
                actual_task_id: another_task_id,
                plan_id: plan_id.clone(),
            })
        );

        let task = repository.get_task(&task_id).unwrap().unwrap();
        assert_eq!(task.phase, TaskPhase::NeedDecision);
        assert_eq!(task.phase_version, 0);
        assert_eq!(repository.get_execution_plan(&plan_id).unwrap(), None);
    }

    /// Job 期待的 Phase 不匹配时，不应该发生任何写入。
    #[test]
    fn should_return_stale_when_job_phase_does_not_match() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let stale_job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedArguments, 0);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&stale_job, plan, TaskPhase::NeedArguments),
            Ok(CommitOutcome::Stale)
        );

        let task = repository.get_task(&task_id).unwrap().unwrap();
        assert_eq!(task.phase, TaskPhase::NeedDecision);
        assert_eq!(task.phase_version, 0);
        assert_eq!(repository.get_execution_plan(&plan_id).unwrap(), None);
    }

    /// 构建执行计划，同时附带执行计划的 id。
    fn build_plan_with_id(task_id: &TaskId, plan_id_text: &str) -> ExecutionPlan {
        let plan_id = ExecutionPlanId::new(plan_id_text);

        let tool_call = ToolCall::new(
            ToolCallId::new(format!("{plan_id_text}-call")),
            plan_id.clone(),
            ToolCallPlan {
                call_key: format!("{plan_id_text}-call"),
                tool_name: "test-tool".to_string(),
                purpose: "测试调用".to_string(),
            },
        );

        ExecutionPlan::try_new(plan_id, task_id.clone(), 0, 0, vec![tool_call])
            .expect("测试计划应满足 ExecutionPlan 的条件")
    }

    /// 两个线程提交相同 Job 时最多只能有一个成功。
    #[test]
    fn should_allow_only_one_concurrent_commit() {
        use std::sync::{Arc, Barrier};

        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("task-1");

        repository
            .create_task(Task::new(task_id.clone(), "测试任务", State::default()))
            .unwrap();

        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let first_plan = build_plan_with_id(&task_id, "plan-a");
        let second_plan = build_plan_with_id(&task_id, "plan-b");
        let first_plan_id = first_plan.id.clone();
        let second_plan_id = second_plan.id.clone();

        let barrier = Arc::new(Barrier::new(3));

        let first_repository = repository.clone();
        let first_job = job.clone();
        let first_barrier = barrier.clone();
        let first_handle = std::thread::spawn(move || {
            first_barrier.wait();
            first_repository.commit_plan_and_transition(
                &first_job,
                first_plan,
                TaskPhase::NeedArguments,
            )
        });

        let second_repository = repository.clone();
        let second_job = job;
        let second_barrier = barrier.clone();
        let second_handle = std::thread::spawn(move || {
            second_barrier.wait();
            second_repository.commit_plan_and_transition(
                &second_job,
                second_plan,
                TaskPhase::NeedArguments,
            )
        });

        barrier.wait();

        let first_result = first_handle.join().unwrap();
        let second_result = second_handle.join().unwrap();

        assert!(
            (first_result == Ok(CommitOutcome::Committed)
                && second_result == Ok(CommitOutcome::Stale))
                || (first_result == Ok(CommitOutcome::Stale)
                    && second_result == Ok(CommitOutcome::Committed))
        );

        let saved_plan_count = [
            repository
                .get_execution_plan(&first_plan_id)
                .unwrap()
                .is_some(),
            repository
                .get_execution_plan(&second_plan_id)
                .unwrap()
                .is_some(),
        ]
        .into_iter()
        .filter(|saved| *saved)
        .count();

        assert_eq!(saved_plan_count, 1);
    }
}

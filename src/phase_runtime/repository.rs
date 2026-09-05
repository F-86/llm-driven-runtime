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
            ExecutionPlan, ExecutionPlanId, InvalidPhaseTransition, Task, TaskId, TaskPhase,
            ToolCall, ToolCallId, ToolCallPlan,
        },
    };

    use super::{CommitOutcome, InMemoryRepository, RepositoryError};

    /// 创建一个 `Task`。
    fn new_task(task_id: &str) -> Task {
        Task::new(TaskId::new(task_id), "测试任务", State::default())
    }

    /// 强制把 `Task` 保存到 `InMemoryRepository` 中。
    fn create_task(repository: &InMemoryRepository, task: &Task) {
        repository
            .create_task(task.clone())
            .expect("创建 Task 应该成功");
    }

    /// 强制获取 `InMemoryRepository` 中保存的 `Task`。
    fn get_saved_task(repository: &InMemoryRepository, task_id: &TaskId) -> Task {
        repository
            .get_task(task_id)
            .expect("读取 Task 应该成功")
            .expect("Task 应该存在")
    }

    /// 强制获取 `InMemoryRepository` 中保存的 `ExecutionPlan`。
    fn get_saved_plan(
        repository: &InMemoryRepository,
        plan_id: &ExecutionPlanId,
    ) -> Option<ExecutionPlan> {
        repository
            .get_execution_plan(plan_id)
            .expect("读取执行计划应该成功")
    }

    /// 断言任务和执行计划没有改变。
    ///
    /// 任务仍是之前的任务，执行计划应该为空。
    fn assert_task_and_plan_unchanged(
        repository: &InMemoryRepository,
        original_task: &Task,
        plan_id: &ExecutionPlanId,
    ) {
        assert_eq!(
            get_saved_task(repository, &original_task.id),
            original_task.clone()
        );
        assert_eq!(get_saved_plan(repository, plan_id), None);
    }

    /// 验证创建后的 Task 可以按 id 完整读取。
    ///
    /// 方法：创建一个新 Task，再按其 id 读取并比较完整值。
    #[test]
    fn should_create_and_read_task() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");

        create_task(&repository, &task);

        assert_eq!(get_saved_task(&repository, &task.id), task);
    }

    /// 验证 Repository 会拒绝 id 重复的 Task。
    ///
    /// 方法：连续创建两个相同 id 的 Task，并比较第二次调用的错误。
    #[test]
    fn should_reject_duplicate_task() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");

        create_task(&repository, &task);

        assert_eq!(
            repository.create_task(task.clone()),
            Err(RepositoryError::TaskAlreadyExists(task.id.clone()))
        );
    }

    /// 验证锁中毒后 Repository 返回明确错误。
    ///
    /// 方法：在线程持锁期间故意 panic，再通过公开读取接口检查错误。
    #[test]
    fn should_return_error_when_lock_is_poisoned() {
        let repository = InMemoryRepository::new();
        let repository_for_panic = repository.clone();

        let result = std::thread::spawn(move || {
            let _guard = repository_for_panic
                .inner
                .lock()
                .expect("测试线程应该获得 Repository 锁");

            panic!("故意让 Repository 锁中毒");
        })
        .join();
        assert!(result.is_err(), "测试线程应该因故意 panic 结束");

        assert_eq!(
            repository.get_task(&TaskId::new("task-1")),
            Err(RepositoryError::LockPoisoned)
        );
    }

    /// 验证读取不存在的执行计划会返回 `None`。
    ///
    /// 方法：在空 Repository 中按未知 id 读取执行计划。
    #[test]
    fn should_return_none_when_execution_plan_does_not_exist() {
        let repository = InMemoryRepository::new();

        assert_eq!(
            get_saved_plan(&repository, &ExecutionPlanId::new("plan-1")),
            None
        );
    }

    /// 强制创建一个 `ExecutionPlan`，附带计划的 id。
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

    /// 强制创建一个 `ExecutionPlan`，计划 id 默认为 "plan-1"。
    fn build_plan(task_id: &TaskId) -> ExecutionPlan {
        build_plan_with_id(task_id, "plan-1")
    }

    /// 验证成功提交会同时保存完整计划并推进 Task。
    ///
    /// 方法：提交匹配的 Job 和有效计划，再读取两个持久化对象进行比较。
    #[test]
    fn should_commit_plan_and_advance_task_atomically() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");
        create_task(&repository, &task);

        let job = RuntimeJob::new(task.id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task.id);
        let plan_id = plan.id.clone();
        let expected_plan = plan.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Ok(CommitOutcome::Committed)
        );

        let saved_task = get_saved_task(&repository, &task.id);
        assert_eq!(
            (
                saved_task.phase,
                saved_task.phase_version,
                saved_task.state_version,
                saved_task.scheduling_status,
            ),
            (TaskPhase::NeedArguments, 1, 0, SchedulingStatus::Ready)
        );
        assert_eq!(get_saved_plan(&repository, &plan_id), Some(expected_plan));
    }

    /// 验证非法 Phase 迁移不会产生部分提交。
    ///
    /// 方法：提交不能从当前 Phase 到达的目标，并比较错误和持久化前后快照。
    #[test]
    fn should_not_partially_commit_when_transition_is_invalid() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");
        create_task(&repository, &task);

        let job = RuntimeJob::new(task.id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task.id);
        let plan_id = plan.id.clone();

        let result = repository.commit_plan_and_transition(&job, plan, TaskPhase::ReadyToExecute);

        assert!(matches!(
            result,
            Err(RepositoryError::InvalidPhaseTransition(
                InvalidPhaseTransition {
                    from: TaskPhase::NeedDecision,
                    to: TaskPhase::ReadyToExecute,
                }
            ))
        ));

        assert_task_and_plan_unchanged(&repository, &task, &plan_id);
    }

    /// 验证 Phase 或版本不匹配的 Job 都会返回 `Stale` 且不写入。
    ///
    /// 方法：表驱动地构造两类不匹配 Job，并比较结果及持久化前后快照。
    #[test]
    fn should_return_stale_for_mismatched_job_without_writing() {
        for (case_name, expected_phase, expected_version) in [
            ("版本不匹配", TaskPhase::NeedDecision, 1),
            ("Phase 不匹配", TaskPhase::NeedArguments, 0),
        ] {
            let repository = InMemoryRepository::new();
            let task = new_task("task-1");
            create_task(&repository, &task);

            let stale_job = RuntimeJob::new(task.id.clone(), expected_phase, expected_version);
            let plan = build_plan(&task.id);
            let plan_id = plan.id.clone();

            assert_eq!(
                repository.commit_plan_and_transition(&stale_job, plan, TaskPhase::NeedArguments),
                Ok(CommitOutcome::Stale),
                "{case_name} 应返回 Stale"
            );
            assert_task_and_plan_unchanged(&repository, &task, &plan_id);
        }
    }

    /// 验证重复提交同一 Job 不会覆盖先前保存的输出。
    ///
    /// 方法：先成功提交，再重放相同 Job，并比较两次提交间的 Task 与计划快照。
    #[test]
    fn should_not_commit_same_job_twice() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");
        create_task(&repository, &task);

        let job = RuntimeJob::new(task.id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task.id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan.clone(), TaskPhase::NeedArguments,),
            Ok(CommitOutcome::Committed)
        );

        let task_after_first_commit = get_saved_task(&repository, &task.id);
        let plan_after_first_commit = get_saved_plan(&repository, &plan_id);

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments,),
            Ok(CommitOutcome::Stale)
        );

        assert_eq!(
            get_saved_task(&repository, &task.id),
            task_after_first_commit
        );
        assert_eq!(
            get_saved_plan(&repository, &plan_id),
            plan_after_first_commit
        );
    }

    /// 验证不存在的 Task 会导致提交失败且不保存计划。
    ///
    /// 方法：不创建目标 Task 就提交计划，并比较错误和计划读取结果。
    #[test]
    fn should_return_task_not_found() {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new("missing-task");
        let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Err(RepositoryError::TaskNotFound(task_id))
        );
        assert_eq!(get_saved_plan(&repository, &plan_id), None);
    }

    /// 验证计划属于其他 Task 时会被拒绝且不写入。
    ///
    /// 方法：为另一个 Task 构造计划，提交到当前 Task 的 Job，并比较错误和快照。
    #[test]
    fn should_reject_plan_for_another_task() {
        let repository = InMemoryRepository::new();
        let task = new_task("task-1");
        let another_task_id = TaskId::new("task-2");
        create_task(&repository, &task);

        let job = RuntimeJob::new(task.id.clone(), TaskPhase::NeedDecision, 0);
        let plan = build_plan(&another_task_id);
        let plan_id = plan.id.clone();

        assert_eq!(
            repository.commit_plan_and_transition(&job, plan, TaskPhase::NeedArguments),
            Err(RepositoryError::PlanTaskMismatch {
                expected_task_id: task.id.clone(),
                actual_task_id: another_task_id,
                plan_id: plan_id.clone(),
            })
        );
        assert_task_and_plan_unchanged(&repository, &task, &plan_id);
    }

    /// 验证两个线程竞争提交同一 Job 时最多只有一个提交成功。
    ///
    /// 方法：用 Barrier 同步两个提交线程，并检查结果组合和保存计划数量。
    #[test]
    fn should_allow_only_one_concurrent_commit() {
        use std::sync::{Arc, Barrier};

        let repository = InMemoryRepository::new();
        let task = new_task("task-1");
        create_task(&repository, &task);

        let job = RuntimeJob::new(task.id.clone(), TaskPhase::NeedDecision, 0);
        let first_plan = build_plan_with_id(&task.id, "plan-a");
        let second_plan = build_plan_with_id(&task.id, "plan-b");
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

        let first_result = first_handle.join().expect("第一个提交线程不应 panic");
        let second_result = second_handle.join().expect("第二个提交线程不应 panic");

        assert!(
            matches!(
                (first_result, second_result),
                (Ok(CommitOutcome::Committed), Ok(CommitOutcome::Stale))
                    | (Ok(CommitOutcome::Stale), Ok(CommitOutcome::Committed))
            ),
            "一个提交应成功，另一个应因 Job 过期返回 Stale"
        );

        let saved_plan_count = [first_plan_id, second_plan_id]
            .into_iter()
            .filter(|plan_id| get_saved_plan(&repository, plan_id).is_some())
            .count();

        assert_eq!(saved_plan_count, 1);
    }
}

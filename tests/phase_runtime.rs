use std::sync::{Arc, Barrier};

use llm_driven_runtime::{
    phase_runtime::{InMemoryRepository, PhaseRuntime, RepositoryError, RuntimeError},
    runtime::HandleOutcome,
    scheduling::RuntimeJob,
    state::State,
    task::{
        Decision, ExecutionPlan, ExecutionPlanError, ExecutionPlanId, ExecutionPlanStatus, Task,
        TaskId, TaskPhase, ToolCallId, ToolCallPlan, ToolCallStatus,
    },
    tool::{SideEffect, ToolMetadata, registry::ToolRegistry},
};

mod phase_fixtures;

use phase_fixtures::{FixedDecisionHandler, MetadataTool, UnexpectedDecisionHandler};

const TASK_ID_TEXT: &str = "task-1";
/// 只读工具 1 号：`get_runtime_status`
const GET_RUNTIME_STATUS: &str = "get_runtime_status";
/// 只读工具 2 号：`query_task`
const QUERY_TASK: &str = "query_task";
/// 写工具：`update_task`
const UPDATE_TASK: &str = "update_task";

/// 使用传入的测试工具构建注册表。
fn tool_registry_with(tools: impl IntoIterator<Item = MetadataTool>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    for tool in tools {
        registry.register(tool);
    }

    registry
}

/// 构建包含两个只读工具的默认注册表。
fn default_tool_registry() -> ToolRegistry {
    tool_registry_with([
        MetadataTool::new(GET_RUNTIME_STATUS, ToolMetadata::read_only(Vec::new())),
        MetadataTool::new(QUERY_TASK, ToolMetadata::read_only(Vec::new())),
    ])
}

/// 构建一个写工具和一个只读工具共存的注册表。
fn write_and_read_tool_registry() -> ToolRegistry {
    tool_registry_with([
        MetadataTool::new(
            UPDATE_TASK,
            ToolMetadata {
                side_effect: SideEffect::Write,
                requires_approval: false,
                read_resources: Vec::new(),
                write_resources: Vec::new(),
            },
        ),
        MetadataTool::new(GET_RUNTIME_STATUS, ToolMetadata::read_only(Vec::new())),
    ])
}

/// 为测试创建初始的 `NeedDecision` Task。
fn create_need_decision_task(repository: &InMemoryRepository, task_id: &TaskId) -> Task {
    let task = Task::new(task_id.clone(), "测试任务", State::default());
    repository
        .create_task(task.clone())
        .expect("创建 Task 应该成功");

    task
}

/// 创建初始阶段匹配的 Runtime Job。
fn need_decision_job(task_id: &TaskId) -> RuntimeJob {
    RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0)
}

/// 生成给定阶段版本对应的 `ExecutionPlan` id。
fn execution_plan_id(task_id: &TaskId, phase_version: u64) -> ExecutionPlanId {
    ExecutionPlanId::new(format!("{task_id}:plan:{phase_version}"))
}

/// 创建一个候选工具调用。
fn tool_call_plan(call_key: &str, tool_name: &str) -> ToolCallPlan {
    ToolCallPlan {
        call_key: call_key.to_string(),
        tool_name: tool_name.to_string(),
        purpose: format!("测试调用 {tool_name}"),
    }
}

/// 创建默认的单只读工具决策。
fn default_decision() -> Decision {
    Decision::NeedToolCall {
        tool_call_plans: vec![tool_call_plan("query-status", GET_RUNTIME_STATUS)],
    }
}

/// 使用固定决策处理器构建 Phase Runtime。
fn phase_runtime(
    repository: InMemoryRepository,
    tool_registry: ToolRegistry,
    decision: Decision,
) -> PhaseRuntime<FixedDecisionHandler> {
    PhaseRuntime::new(
        repository,
        tool_registry,
        FixedDecisionHandler::new(decision),
    )
}

/// 使用默认只读注册表构建 Phase Runtime。
fn default_phase_runtime(
    repository: InMemoryRepository,
    decision: Decision,
) -> PhaseRuntime<FixedDecisionHandler> {
    phase_runtime(repository, default_tool_registry(), decision)
}

/// 读取已保存的 Task，缺失时使测试失败。
fn get_saved_task(repository: &InMemoryRepository, task_id: &TaskId) -> Task {
    repository
        .get_task(task_id)
        .expect("读取 Task 应该成功")
        .expect("Task 应该存在")
}

/// 读取已保存的 ExecutionPlan，缺失时使测试失败。
fn get_saved_plan(repository: &InMemoryRepository, plan_id: &ExecutionPlanId) -> ExecutionPlan {
    repository
        .get_execution_plan(plan_id)
        .expect("读取 ExecutionPlan 应该成功")
        .expect("ExecutionPlan 应该存在")
}

/// 验证失败路径没有修改 Task，也没有保存当前阶段的 `ExecutionPlan`。
fn assert_task_and_plan_unchanged(
    repository: &InMemoryRepository,
    task_id: &TaskId,
    expected_task: &Task,
    plan_phase_version: u64,
) {
    assert_eq!(get_saved_task(repository, task_id), expected_task.clone());
    assert_eq!(
        repository
            .get_execution_plan(&execution_plan_id(task_id, plan_phase_version))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 表驱动测试中使用的拒绝规划用例。
struct RejectedPlanCase {
    name: &'static str,
    decision: Decision,
    tool_registry: fn() -> ToolRegistry,
    expected_error: RuntimeError,
}

/// 验证匹配的 `NeedDecision` job 会生成单调用计划并推进阶段。
///
/// 方法：固定决策处理器返回一个已注册只读工具，再读取持久化的 Task 与 `ExecutionPlan`。
#[test]
fn should_create_planned_execution_plan_from_matching_need_decision_job() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let expected_call_plan = tool_call_plan("query-status", GET_RUNTIME_STATUS);
    let runtime = default_phase_runtime(
        repository.clone(),
        Decision::NeedToolCall {
            tool_call_plans: vec![expected_call_plan.clone()],
        },
    );

    assert_eq!(
        runtime.handle(&need_decision_job(&task_id)),
        Ok(HandleOutcome::PhaseAdvanced)
    );

    let task = get_saved_task(&repository, &task_id);
    assert_eq!(task.phase, TaskPhase::NeedArguments);
    assert_eq!(task.phase_version, 1);
    assert_eq!(task.state_version, 0);

    let plan = get_saved_plan(&repository, &execution_plan_id(&task_id, 0));
    assert_eq!(plan.task_id, task_id);
    assert_eq!(plan.ordinal, 0);
    assert_eq!(plan.state_version, 0);
    assert_eq!(plan.status(), ExecutionPlanStatus::Planned);
    assert_eq!(plan.tool_calls.len(), 1);
    assert_eq!(plan.tool_calls[0].plan, expected_call_plan);
    assert_eq!(
        plan.tool_calls[0].execution.status(),
        ToolCallStatus::ArgumentsPending
    );
}

/// 验证多个候选调用会保序映射到同一个 `ExecutionPlan`。
///
/// 方法：固定决策返回两个不同的只读工具，并检查生成的 `ToolCall` id、计划归属和调用顺序。
#[test]
fn should_preserve_multiple_calls_in_one_execution_plan() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let expected_call_plans = vec![
        tool_call_plan("query-status", GET_RUNTIME_STATUS),
        tool_call_plan("query-task", QUERY_TASK),
    ];
    let runtime = default_phase_runtime(
        repository.clone(),
        Decision::NeedToolCall {
            tool_call_plans: expected_call_plans.clone(),
        },
    );

    assert_eq!(
        runtime.handle(&need_decision_job(&task_id)),
        Ok(HandleOutcome::PhaseAdvanced)
    );

    let plan_id = execution_plan_id(&task_id, 0);
    let plan = get_saved_plan(&repository, &plan_id);
    assert_eq!(
        plan.tool_calls
            .iter()
            .map(|tool_call| tool_call.id.clone())
            .collect::<Vec<_>>(),
        vec![
            ToolCallId::new(format!("{plan_id}:call:0")),
            ToolCallId::new(format!("{plan_id}:call:1")),
        ]
    );
    assert!(
        plan.tool_calls
            .iter()
            .all(|tool_call| tool_call.execution_plan_id == plan_id)
    );
    assert_eq!(
        plan.tool_calls
            .iter()
            .map(|tool_call| tool_call.plan.clone())
            .collect::<Vec<_>>(),
        expected_call_plans
    );
}

/// 验证重复的 job 被识别为 Stale 且不会覆盖已保存输出。
///
/// 方法：先成功处理一次 job 并保存快照，再重放同一 job，比较处理前后的 Task 与计划。
#[test]
fn should_return_stale_when_job_is_replayed_without_overwriting_output() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let runtime = default_phase_runtime(repository.clone(), default_decision());
    let job = need_decision_job(&task_id);

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::PhaseAdvanced));
    let task_before_replay = get_saved_task(&repository, &task_id);
    let plan_id = execution_plan_id(&task_id, 0);
    let plan_before_replay = get_saved_plan(&repository, &plan_id);

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::Stale));
    assert_eq!(get_saved_task(&repository, &task_id), task_before_replay);
    assert_eq!(get_saved_plan(&repository, &plan_id), plan_before_replay);
}

/// 验证 phase 或版本不匹配的 job 都会返回 Stale。
///
/// 方法：对同一个初始 Task 依次提交 phase 不匹配和版本不匹配的 job，并验证没有持久化副作用。
#[test]
fn should_return_stale_for_mismatched_job_without_persistence() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    let initial_task = create_need_decision_task(&repository, &task_id);
    let runtime = default_phase_runtime(repository.clone(), default_decision());

    for stale_job in [
        RuntimeJob::new(task_id.clone(), TaskPhase::NeedArguments, 0),
        RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 1),
    ] {
        assert_eq!(runtime.handle(&stale_job), Ok(HandleOutcome::Stale));
    }

    assert_task_and_plan_unchanged(&repository, &task_id, &initial_task, 0);
}

/// 验证所有无法规划的代表性决策都会在提交前被拒绝。
///
/// 方法：用表驱动方式覆盖不支持决策、空计划、重复调用键、未注册工具和多调用写工具，并统一验证原子性。
#[test]
fn should_reject_invalid_decisions_without_persisting() {
    let cases = [
        RejectedPlanCase {
            name: "不支持的决策",
            decision: Decision::Finish,
            tool_registry: default_tool_registry,
            expected_error: RuntimeError::UnsupportedDecision,
        },
        RejectedPlanCase {
            name: "空计划",
            decision: Decision::NeedToolCall {
                tool_call_plans: Vec::new(),
            },
            tool_registry: default_tool_registry,
            expected_error: RuntimeError::ExecutionPlan(ExecutionPlanError::EmptyToolCalls),
        },
        RejectedPlanCase {
            name: "重复调用键",
            decision: Decision::NeedToolCall {
                tool_call_plans: vec![
                    tool_call_plan("inspect", GET_RUNTIME_STATUS),
                    tool_call_plan("inspect", QUERY_TASK),
                ],
            },
            tool_registry: default_tool_registry,
            expected_error: RuntimeError::ExecutionPlan(ExecutionPlanError::DuplicateCallKey(
                "inspect".to_string(),
            )),
        },
        RejectedPlanCase {
            name: "未注册工具",
            decision: Decision::NeedToolCall {
                tool_call_plans: vec![tool_call_plan("unknown", "not_registered")],
            },
            tool_registry: default_tool_registry,
            expected_error: RuntimeError::UnknownTool {
                tool_name: "not_registered".to_string(),
            },
        },
        RejectedPlanCase {
            name: "多调用写工具",
            decision: Decision::NeedToolCall {
                tool_call_plans: vec![
                    tool_call_plan("update", UPDATE_TASK),
                    tool_call_plan("inspect", GET_RUNTIME_STATUS),
                ],
            },
            tool_registry: write_and_read_tool_registry,
            expected_error: RuntimeError::ParallelWriteTool {
                tool_name: UPDATE_TASK.to_string(),
            },
        },
    ];

    for case in cases {
        let repository = InMemoryRepository::new();
        let task_id = TaskId::new(TASK_ID_TEXT);
        let initial_task = create_need_decision_task(&repository, &task_id);
        let runtime = phase_runtime(repository.clone(), (case.tool_registry)(), case.decision);

        assert_eq!(
            runtime.handle(&need_decision_job(&task_id)),
            Err(case.expected_error),
            "{} 应被拒绝",
            case.name
        );
        assert_task_and_plan_unchanged(&repository, &task_id, &initial_task, 0);
    }
}

/// 验证未实现阶段在请求决策前被拒绝。
///
/// 方法：将 Task 准备到 `NeedArguments`，注入会返回错误的决策处理器，并检查 Runtime 仍返回阶段不支持错误。
#[test]
fn should_reject_unimplemented_phase_before_requesting_decision() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    let mut task = Task::new(task_id.clone(), "测试任务", State::default());
    task.transition_to(TaskPhase::NeedArguments)
        .expect("测试准备应允许迁移到 NeedArguments");
    let initial_task = task.clone();
    repository.create_task(task).expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        UnexpectedDecisionHandler,
    );

    let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedArguments, 1);
    assert_eq!(
        runtime.handle(&job),
        Err(RuntimeError::UnsupportedPhase(TaskPhase::NeedArguments))
    );
    assert_task_and_plan_unchanged(&repository, &task_id, &initial_task, 1);
}

/// 验证同一只读工具可以在一个计划中出现多次。
///
/// 方法：固定决策返回两个调用同一工具的不同调用键，并检查两个 `ToolCall` 都被保存。
#[test]
fn should_plan_multiple_calls_to_same_read_only_tool() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let runtime = default_phase_runtime(
        repository.clone(),
        Decision::NeedToolCall {
            tool_call_plans: vec![
                tool_call_plan("first", GET_RUNTIME_STATUS),
                tool_call_plan("second", GET_RUNTIME_STATUS),
            ],
        },
    );

    assert_eq!(
        runtime.handle(&need_decision_job(&task_id)),
        Ok(HandleOutcome::PhaseAdvanced)
    );

    let plan = get_saved_plan(&repository, &execution_plan_id(&task_id, 0));
    let tool_names = plan
        .tool_calls
        .iter()
        .map(|tool_call| tool_call.plan.tool_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tool_names, vec![GET_RUNTIME_STATUS, GET_RUNTIME_STATUS]);
}

/// 验证单个写工具不受多调用并行约束限制。
///
/// 方法：注册一个写工具并让固定决策仅返回该调用，检查 Phase 可以正常推进。
#[test]
fn should_allow_single_write_tool_call() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let runtime = phase_runtime(
        repository.clone(),
        write_and_read_tool_registry(),
        Decision::NeedToolCall {
            tool_call_plans: vec![tool_call_plan("update", UPDATE_TASK)],
        },
    );

    assert_eq!(
        runtime.handle(&need_decision_job(&task_id)),
        Ok(HandleOutcome::PhaseAdvanced)
    );
}

/// 验证读取不到 Task 时返回明确的 Repository 错误。
///
/// 方法：不创建 Task，直接提交一个匹配初始版本的 job 并比较错误中的 Task id。
#[test]
fn should_return_task_not_found_for_unknown_task_id() {
    let repository = InMemoryRepository::new();
    let missing_task_id = TaskId::new("missing-task");
    let runtime = default_phase_runtime(repository, default_decision());

    assert_eq!(
        runtime.handle(&need_decision_job(&missing_task_id)),
        Err(RuntimeError::Repository(RepositoryError::TaskNotFound(
            missing_task_id
        )))
    );
}

/// 验证两个线程处理同一 job 时最多只有一个提交成功。
///
/// 方法：用 Barrier 同步两个线程同时调用同一个 Runtime，再检查结果组合和最终持久化状态。
#[test]
fn should_allow_only_one_concurrent_phase_runtime_commit() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new(TASK_ID_TEXT);
    create_need_decision_task(&repository, &task_id);
    let runtime = Arc::new(default_phase_runtime(
        repository.clone(),
        default_decision(),
    ));
    let job = need_decision_job(&task_id);
    let barrier = Arc::new(Barrier::new(3));

    let first_runtime = runtime.clone();
    let first_job = job.clone();
    let first_barrier = barrier.clone();
    let first_handle = std::thread::spawn(move || {
        first_barrier.wait();
        first_runtime.handle(&first_job)
    });

    let second_runtime = runtime;
    let second_barrier = barrier.clone();
    let second_handle = std::thread::spawn(move || {
        second_barrier.wait();
        second_runtime.handle(&job)
    });

    barrier.wait();

    let first_outcome = first_handle.join().expect("第一个线程应正常结束");
    let second_outcome = second_handle.join().expect("第二个线程应正常结束");
    assert!(
        (first_outcome == Ok(HandleOutcome::PhaseAdvanced)
            && second_outcome == Ok(HandleOutcome::Stale))
            || (first_outcome == Ok(HandleOutcome::Stale)
                && second_outcome == Ok(HandleOutcome::PhaseAdvanced))
    );

    let task = get_saved_task(&repository, &task_id);
    assert_eq!(task.phase, TaskPhase::NeedArguments);
    assert_eq!(task.phase_version, 1);
    assert_eq!(
        get_saved_plan(&repository, &execution_plan_id(&task_id, 0))
            .tool_calls
            .len(),
        1
    );
}

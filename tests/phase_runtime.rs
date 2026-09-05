use llm_driven_runtime::{
    phase_runtime::{InMemoryRepository, PhaseRuntime, RepositoryError, RuntimeError},
    runtime::HandleOutcome,
    scheduling::RuntimeJob,
    state::State,
    task::{
        Decision, ExecutionPlanError, ExecutionPlanId, ExecutionPlanStatus, Task, TaskId,
        TaskPhase, ToolCallId, ToolCallPlan, ToolCallStatus,
    },
    tool::{SideEffect, ToolMetadata, registry::ToolRegistry},
};

#[path = "common/decision_handler.rs"]
mod decision_handler;
#[path = "common/tool/phase_runtime_tool.rs"]
mod phase_runtime_tool;

use decision_handler::FixedDecisionHandler;
use phase_runtime_tool::MetadataTool;

/// 使用传入的参数构建一个 `ToolRegistry`。
fn tool_registry_with(tools: impl IntoIterator<Item = MetadataTool>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    for tool in tools {
        registry.register(tool);
    }

    registry
}

/// 默认的 `ToolRegistry`。
fn default_tool_registry() -> ToolRegistry {
    tool_registry_with([
        MetadataTool::new("get_runtime_status", ToolMetadata::read_only(Vec::new())),
        MetadataTool::new("query_task", ToolMetadata::read_only(Vec::new())),
    ])
}

/// 匹配的 `NeedDecision` job 应原子保存一个计划并推进一个 Phase。
#[test]
fn should_plan_single_tool_call_and_advance_need_decision() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository.create_task(task).expect("创建 Task 应该成功");

    let tool_call_plan = ToolCallPlan {
        call_key: "query-status".to_string(),
        tool_name: "get_runtime_status".to_string(),
        purpose: "获取当前运行时状态".to_string(),
    };
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![tool_call_plan.clone()],
        }),
    );
    let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::PhaseAdvanced));

    let saved_task = repository
        .get_task(&task_id)
        .expect("读取 Task 应该成功")
        .expect("Task 应该存在");
    assert_eq!(saved_task.phase, TaskPhase::NeedArguments);
    assert_eq!(saved_task.phase_version, 1);
    assert_eq!(saved_task.state_version, 0);

    let plan_id = ExecutionPlanId::new("task-1:plan:0");
    let saved_plan = repository
        .get_execution_plan(&plan_id)
        .expect("读取 ExecutionPlan 应该成功")
        .expect("ExecutionPlan 应该存在");
    assert_eq!(saved_plan.task_id, task_id);
    assert_eq!(saved_plan.ordinal, 0);
    assert_eq!(saved_plan.state_version, 0);
    assert_eq!(saved_plan.status(), ExecutionPlanStatus::Planned);
    assert_eq!(saved_plan.tool_calls.len(), 1);
    assert_eq!(saved_plan.tool_calls[0].plan, tool_call_plan);
    assert_eq!(
        saved_plan.tool_calls[0].execution.status(),
        ToolCallStatus::ArgumentsPending
    );
}

/// 匹配且经工具元数据校验的决策可以在同一个 `ExecutionPlan` 中保存多个工具调用。
#[test]
fn should_plan_multiple_tool_calls_and_advance_need_decision() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    repository
        .create_task(Task::new(
            task_id.clone(),
            "查询任务与运行时状态",
            State::default(),
        ))
        .expect("创建 Task 应该成功");

    let tool_call_plans = vec![
        ToolCallPlan {
            call_key: "query-status".to_string(),
            tool_name: "get_runtime_status".to_string(),
            purpose: "获取当前运行时状态".to_string(),
        },
        ToolCallPlan {
            call_key: "query-task".to_string(),
            tool_name: "query_task".to_string(),
            purpose: "查询任务".to_string(),
        },
    ];
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: tool_call_plans.clone(),
        }),
    );
    let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::PhaseAdvanced));

    let saved_task = repository
        .get_task(&task_id)
        .expect("读取 Task 应该成功")
        .expect("Task 应该存在");
    assert_eq!(saved_task.phase, TaskPhase::NeedArguments);
    assert_eq!(saved_task.phase_version, 1);
    assert_eq!(saved_task.state_version, 0);

    let plan_id = ExecutionPlanId::new("task-1:plan:0");
    let saved_plan = repository
        .get_execution_plan(&plan_id)
        .expect("读取 ExecutionPlan 应该成功")
        .expect("ExecutionPlan 应该存在");
    assert_eq!(saved_plan.status(), ExecutionPlanStatus::Planned);
    assert_eq!(saved_plan.tool_calls.len(), 2);
    assert_eq!(
        saved_plan.tool_calls[0].id,
        ToolCallId::new("task-1:plan:0:call:0")
    );
    assert_eq!(
        saved_plan.tool_calls[1].id,
        ToolCallId::new("task-1:plan:0:call:1")
    );
    assert!(
        saved_plan
            .tool_calls
            .iter()
            .all(|tool_call| tool_call.execution_plan_id == plan_id)
    );
    assert_eq!(
        saved_plan
            .tool_calls
            .iter()
            .map(|tool_call| tool_call.plan.clone())
            .collect::<Vec<_>>(),
        tool_call_plans
    );
    assert!(
        saved_plan
            .tool_calls
            .iter()
            .all(|tool_call| tool_call.execution.status() == ToolCallStatus::ArgumentsPending)
    );
}

/// 同一个 job 重放时必须被识别为过期，且不能覆盖已经保存的输出。
#[test]
fn should_return_stale_without_changing_task_or_plan_for_replayed_job() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    repository
        .create_task(Task::new(
            task_id.clone(),
            "检查运行时状态",
            State::default(),
        ))
        .expect("创建 Task 应该成功");

    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "query-status".to_string(),
                tool_name: "get_runtime_status".to_string(),
                purpose: "获取当前运行时状态".to_string(),
            }],
        }),
    );
    let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::PhaseAdvanced));
    let saved_task = repository
        .get_task(&task_id)
        .expect("读取 Task 应该成功")
        .expect("Task 应该存在");
    let plan_id = ExecutionPlanId::new("task-1:plan:0");
    let saved_plan = repository
        .get_execution_plan(&plan_id)
        .expect("读取 ExecutionPlan 应该成功")
        .expect("ExecutionPlan 应该存在");

    assert_eq!(runtime.handle(&job), Ok(HandleOutcome::Stale));
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(saved_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&plan_id)
            .expect("读取 ExecutionPlan 应该成功"),
        Some(saved_plan)
    );
}

/// 不匹配的 Phase 或 version 不应调用成功路径，也不应创建 `ExecutionPlan`。
#[test]
fn should_return_stale_without_writes_for_mismatched_phase_or_version() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");

    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "query-status".to_string(),
                tool_name: "get_runtime_status".to_string(),
                purpose: "获取当前运行时状态".to_string(),
            }],
        }),
    );

    for stale_job in [
        RuntimeJob::new(task_id.clone(), TaskPhase::NeedArguments, 0),
        RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 1),
    ] {
        assert_eq!(runtime.handle(&stale_job), Ok(HandleOutcome::Stale));
    }

    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 不支持的决策不得部分保存计划或推进 Task。
#[test]
fn should_reject_unsupported_decision_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::Finish),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Err(RuntimeError::UnsupportedDecision)
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 当前未实现的 Phase 应在调用决策处理器之前明确拒绝，且不改变 Task。
#[test]
fn should_reject_unimplemented_phase_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let mut task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    task.transition_to(TaskPhase::NeedArguments)
        .expect("测试准备应允许迁移到 NeedArguments");
    let saved_task = task.clone();
    repository.create_task(task).expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::Finish),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedArguments,
            1
        )),
        Err(RuntimeError::UnsupportedPhase(TaskPhase::NeedArguments))
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(saved_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:1"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 空的 `NeedToolCall` 决策不应推进 Task 或创建 `ExecutionPlan`。
#[test]
fn should_reject_empty_tool_call_plans_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: Vec::new(),
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Err(RuntimeError::ExecutionPlan(
            ExecutionPlanError::EmptyToolCalls
        ))
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 重复的 `call_key` 不能生成无法稳定引用的执行计划。
#[test]
fn should_reject_duplicate_call_keys_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![
                ToolCallPlan {
                    call_key: "inspect".to_string(),
                    tool_name: "get_runtime_status".to_string(),
                    purpose: "获取当前运行时状态".to_string(),
                },
                ToolCallPlan {
                    call_key: "inspect".to_string(),
                    tool_name: "query_task".to_string(),
                    purpose: "查询任务".to_string(),
                },
            ],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Err(RuntimeError::ExecutionPlan(
            ExecutionPlanError::DuplicateCallKey("inspect".to_string())
        ))
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 工具计划必须引用已注册的工具，不能把不存在的名称持久化为待执行调用。
#[test]
fn should_reject_unknown_tool_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "检查运行时状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "unknown".to_string(),
                tool_name: "not_registered".to_string(),
                purpose: "验证工具存在性".to_string(),
            }],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Err(RuntimeError::UnknownTool {
            tool_name: "not_registered".to_string(),
        })
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 保守并行规则要求多调用计划中的所有工具都是只读的。
#[test]
fn should_reject_write_tool_in_multi_call_plan_without_writes() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    let initial_task = Task::new(task_id.clone(), "更新并检查状态", State::default());
    repository
        .create_task(initial_task.clone())
        .expect("创建 Task 应该成功");

    let mut write_metadata = ToolMetadata::read_only(Vec::new());
    write_metadata.side_effect = SideEffect::Write;
    let runtime = PhaseRuntime::new(
        repository.clone(),
        tool_registry_with([
            MetadataTool::new("update_task", write_metadata),
            MetadataTool::new("get_runtime_status", ToolMetadata::read_only(Vec::new())),
        ]),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![
                ToolCallPlan {
                    call_key: "update".to_string(),
                    tool_name: "update_task".to_string(),
                    purpose: "更新任务".to_string(),
                },
                ToolCallPlan {
                    call_key: "inspect".to_string(),
                    tool_name: "get_runtime_status".to_string(),
                    purpose: "获取当前运行时状态".to_string(),
                },
            ],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Err(RuntimeError::ParallelWriteTool {
            tool_name: "update_task".to_string(),
        })
    );
    assert_eq!(
        repository.get_task(&task_id).expect("读取 Task 应该成功"),
        Some(initial_task)
    );
    assert_eq!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功"),
        None
    );
}

/// 同一只读工具的多个调用可以进入同一计划；实际并发度由未来执行器决定。
#[test]
fn should_plan_multiple_calls_to_same_read_only_tool() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    repository
        .create_task(Task::new(
            task_id.clone(),
            "检查运行时状态",
            State::default(),
        ))
        .expect("创建 Task 应该成功");
    let runtime = PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![
                ToolCallPlan {
                    call_key: "first".to_string(),
                    tool_name: "get_runtime_status".to_string(),
                    purpose: "第一次读取".to_string(),
                },
                ToolCallPlan {
                    call_key: "second".to_string(),
                    tool_name: "get_runtime_status".to_string(),
                    purpose: "第二次读取".to_string(),
                },
            ],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Ok(HandleOutcome::PhaseAdvanced)
    );
    let saved_plan = repository
        .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
        .expect("读取 ExecutionPlan 应该成功")
        .expect("ExecutionPlan 应该存在");
    assert_eq!(saved_plan.status(), ExecutionPlanStatus::Planned);
    assert_eq!(saved_plan.tool_calls.len(), 2);
    assert!(
        saved_plan
            .tool_calls
            .iter()
            .all(|tool_call| tool_call.plan.tool_name == "get_runtime_status")
    );
}

/// 写工具作为计划中的唯一调用时不受保守并行规则限制。
#[test]
fn should_allow_single_write_tool_call() {
    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    repository
        .create_task(Task::new(task_id.clone(), "更新任务", State::default()))
        .expect("创建 Task 应该成功");

    let mut write_metadata = ToolMetadata::read_only(Vec::new());
    write_metadata.side_effect = SideEffect::Write;
    let runtime = PhaseRuntime::new(
        repository.clone(),
        tool_registry_with([MetadataTool::new("update_task", write_metadata)]),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "update".to_string(),
                tool_name: "update_task".to_string(),
                purpose: "更新任务".to_string(),
            }],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            task_id.clone(),
            TaskPhase::NeedDecision,
            0
        )),
        Ok(HandleOutcome::PhaseAdvanced)
    );
    assert_eq!(
        repository
            .get_task(&task_id)
            .expect("读取 Task 应该成功")
            .expect("Task 应该存在")
            .phase,
        TaskPhase::NeedArguments
    );
}

/// 不存在的 Task 应返回明确的读取错误，而不是尝试创建输出。
#[test]
fn should_return_task_not_found_for_unknown_task_id() {
    let repository = InMemoryRepository::new();
    let missing_task_id = TaskId::new("missing-task");
    let runtime = PhaseRuntime::new(
        repository,
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "query-status".to_string(),
                tool_name: "get_runtime_status".to_string(),
                purpose: "获取当前运行时状态".to_string(),
            }],
        }),
    );

    assert_eq!(
        runtime.handle(&RuntimeJob::new(
            missing_task_id.clone(),
            TaskPhase::NeedDecision,
            0,
        )),
        Err(RuntimeError::Repository(RepositoryError::TaskNotFound(
            missing_task_id
        )))
    );
}

/// 两个线程同时处理相同 job 时，最多只能有一个线程推进 Phase。
#[test]
fn should_allow_only_one_concurrent_phase_runtime_commit() {
    use std::sync::{Arc, Barrier};

    let repository = InMemoryRepository::new();
    let task_id = TaskId::new("task-1");
    repository
        .create_task(Task::new(
            task_id.clone(),
            "检查运行时状态",
            State::default(),
        ))
        .expect("创建 Task 应该成功");
    let runtime = Arc::new(PhaseRuntime::new(
        repository.clone(),
        default_tool_registry(),
        FixedDecisionHandler::new(Decision::NeedToolCall {
            tool_call_plans: vec![ToolCallPlan {
                call_key: "query-status".to_string(),
                tool_name: "get_runtime_status".to_string(),
                purpose: "获取当前运行时状态".to_string(),
            }],
        }),
    ));
    let job = RuntimeJob::new(task_id.clone(), TaskPhase::NeedDecision, 0);
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

    let saved_task = repository
        .get_task(&task_id)
        .expect("读取 Task 应该成功")
        .expect("Task 应该存在");
    assert_eq!(saved_task.phase, TaskPhase::NeedArguments);
    assert_eq!(saved_task.phase_version, 1);
    assert!(
        repository
            .get_execution_plan(&ExecutionPlanId::new("task-1:plan:0"))
            .expect("读取 ExecutionPlan 应该成功")
            .is_some()
    );
}

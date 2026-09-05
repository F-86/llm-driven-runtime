use std::fmt;

use crate::{
    runtime::HandleOutcome,
    scheduling::RuntimeJob,
    task::{
        Decision, ExecutionPlan, ExecutionPlanError, ExecutionPlanId, Task, TaskPhase, ToolCall,
        ToolCallId, ToolCallPlan,
    },
    tool::{SideEffect, registry::ToolRegistry},
};

use super::{
    CommitOutcome, DecisionHandler, DecisionHandlerError, InMemoryRepository, RepositoryError,
};

/// 最小 Phase 状态机运行时。
///
/// 这个运行时独立于当前单工具 `Runtime` 原型。
/// 它在一次调用中最多推进一个 `TaskPhase`，并通过 `InMemoryRepository` 原子保存阶段输出与 Task 的迁移。
pub struct PhaseRuntime<H> {
    repository: InMemoryRepository,
    tool_registry: ToolRegistry,
    decision_handler: H,
}

impl<H> PhaseRuntime<H>
where
    H: DecisionHandler,
{
    /// 使用 Repository、工具注册表和决策处理器创建 Phase Runtime。
    #[must_use]
    pub fn new(
        repository: InMemoryRepository,
        tool_registry: ToolRegistry,
        decision_handler: H,
    ) -> Self {
        Self {
            repository,
            tool_registry,
            decision_handler,
        }
    }

    /// 处理一个 Runtime Job。
    ///
    /// 当前最小切片只支持 `NeedDecision`。
    /// 匹配的任务会由决策处理器生成一个或多个工具调用计划，并原子推进到
    /// `NeedArguments`。
    ///
    /// # Errors
    ///
    /// 如果任务不存在，则返回 [`RuntimeError::Repository`]；
    /// 如果任务的阶段不是 [`TaskPhase::NeedDecision`]，则返回 [`RuntimeError::UnsupportedPhase`]；
    /// 如果决策处理器得出的决策不是 [`Decision::NeedToolCall`]，则返回 [`RuntimeError::UnsupportedDecision`]；
    /// 如果计划引用未注册的工具，则返回 [`RuntimeError::UnknownTool`]；
    /// 如果多调用计划包含写工具，则返回 [`RuntimeError::ParallelWriteTool`]；
    /// 如果候选工具调用无法构成有效的 `ExecutionPlan`，则返回 [`RuntimeError::ExecutionPlan`]；
    /// 如果 [`InMemoryRepository::commit_plan_and_transition`] 执行失败，则返回 [`RuntimeError::Repository`]。
    pub fn handle(&self, job: &RuntimeJob) -> Result<HandleOutcome, RuntimeError> {
        let task = self
            .repository
            .get_task(&job.task_id)?
            .ok_or_else(|| RepositoryError::TaskNotFound(job.task_id.clone()))?;

        if !task.matches_job(job) {
            return Ok(HandleOutcome::Stale);
        }

        if task.phase != TaskPhase::NeedDecision {
            return Err(RuntimeError::UnsupportedPhase(task.phase));
        }

        let tool_call_plans = self.tool_call_plans(&task)?;
        self.validate_parallel_safety(&tool_call_plans)?;
        let execution_plan = Self::execution_plan_from(&task, tool_call_plans)?;

        match self.repository.commit_plan_and_transition(
            job,
            execution_plan,
            TaskPhase::NeedArguments,
        )? {
            CommitOutcome::Committed => Ok(HandleOutcome::PhaseAdvanced),
            CommitOutcome::Stale => Ok(HandleOutcome::Stale),
        }
    }

    /// 从决策处理器结果中提取当前切片支持的工具调用计划集合。
    fn tool_call_plans(&self, task: &Task) -> Result<Vec<ToolCallPlan>, RuntimeError> {
        match self.decision_handler.decide(task)? {
            Decision::NeedToolCall { tool_call_plans } => Ok(tool_call_plans),
            Decision::Finish | Decision::NeedUserInput | Decision::Abort { .. } => {
                Err(RuntimeError::UnsupportedDecision)
            }
        }
    }

    /// 保守地验证候选工具调用是否能处于同一个并行执行计划中。
    ///
    /// 当前版本只允许只读工具并行；写工具只能作为计划中的唯一调用。
    /// 资源级冲突判断和实际执行的并发限制由后续执行器负责。
    fn validate_parallel_safety(
        &self,
        tool_call_plans: &[ToolCallPlan],
    ) -> Result<(), RuntimeError> {
        for tool_call_plan in tool_call_plans {
            let tool = self
                .tool_registry
                .get(&tool_call_plan.tool_name)
                .ok_or_else(|| RuntimeError::UnknownTool {
                    tool_name: tool_call_plan.tool_name.clone(),
                })?;
            let metadata = tool.metadata();

            if tool_call_plans.len() > 1 && metadata.side_effect == SideEffect::Write {
                return Err(RuntimeError::ParallelWriteTool {
                    tool_name: tool_call_plan.tool_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// 使用 Task 的稳定标识和当前 Phase 版本构建唯一的最小执行计划。
    ///
    /// 一个 `ToolCallPlan` 对应一个独立的 `ToolCall`。
    /// 调用方必须先通过 [`Self::validate_parallel_safety`] 验证工具元数据约束。
    fn execution_plan_from(
        task: &Task,
        tool_call_plans: Vec<ToolCallPlan>,
    ) -> Result<ExecutionPlan, ExecutionPlanError> {
        let plan_id = ExecutionPlanId::new(format!("{}:plan:{}", task.id, task.phase_version));
        let tool_calls = tool_call_plans
            .into_iter()
            .enumerate()
            .map(|(index, tool_call_plan)| {
                ToolCall::new(
                    ToolCallId::new(format!("{plan_id}:call:{index}")),
                    plan_id.clone(),
                    tool_call_plan,
                )
            })
            .collect();

        // 当前闭环只允许 Task 的第一个 ExecutionPlan，因此 ordinal 固定为 0。
        ExecutionPlan::try_new(plan_id, task.id.clone(), 0, task.state_version, tool_calls)
    }
}

/// Phase Runtime 处理失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// Repository 读取或提交失败。
    Repository(RepositoryError),
    /// 决策处理器无法生成决策。
    DecisionHandler(DecisionHandlerError),
    /// 当前最小切片尚未实现该阶段。
    UnsupportedPhase(TaskPhase),
    /// 当前最小切片只支持 [`Decision::NeedToolCall`]。
    UnsupportedDecision,
    /// 决策处理器引用了未注册的工具。
    UnknownTool {
        /// 未注册的工具名。
        tool_name: String,
    },
    /// 包含多个工具调用的计划引用了写工具。
    ParallelWriteTool {
        /// 写工具名。
        tool_name: String,
    },
    /// 候选工具调用无法构成有效的 `ExecutionPlan`。
    ExecutionPlan(ExecutionPlanError),
}

impl From<RepositoryError> for RuntimeError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<DecisionHandlerError> for RuntimeError {
    fn from(error: DecisionHandlerError) -> Self {
        Self::DecisionHandler(error)
    }
}

impl From<ExecutionPlanError> for RuntimeError {
    fn from(error: ExecutionPlanError) -> Self {
        Self::ExecutionPlan(error)
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "Repository 操作失败：{error}"),
            Self::DecisionHandler(error) => write!(formatter, "{error}"),
            Self::UnsupportedPhase(phase) => {
                write!(formatter, "当前 Phase Runtime 尚未支持阶段：{phase:?}")
            }
            Self::UnsupportedDecision => {
                formatter.write_str("当前 Phase Runtime 只支持 NeedToolCall 决策")
            }
            Self::UnknownTool { tool_name } => {
                write!(formatter, "NeedToolCall 引用了未注册的工具：{tool_name}")
            }
            Self::ParallelWriteTool { tool_name } => {
                write!(formatter, "写工具不能与其他调用并行：{tool_name}")
            }
            Self::ExecutionPlan(error) => write!(formatter, "无法创建 ExecutionPlan：{error}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

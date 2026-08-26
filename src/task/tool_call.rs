use crate::{
    task::{ExecutionPlanId, TaskId, ToolCallId, ToolCallPlan},
    tool::{RetryDecision, ToolError, ToolErrorKind, ToolSuccess},
};

/// `ToolCall` 的执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// 等待参数
    ArgumentsPending,
    /// 参数已就绪
    ArgumentsReady,
    /// 等待用户审批
    WaitingApproval,
    /// 准备执行
    ReadyToExecute,
    /// 正在执行
    Running,
    /// 等待重试
    WaitingRetry,
    /// 执行成功
    Succeeded,
    /// 执行失败
    Failed,
    /// 执行请求可能已经发出，但结果无法确认
    ExecutionUnknown,
    /// 工具调用已失效，不再执行
    Invalidated,
}

impl ToolCallStatus {
    /// 判断工具调用是否已经结束
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::ExecutionUnknown | Self::Invalidated
        )
    }
}

/// 工具调用的执行错误记录
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolErrorRecord {
    /// 错误类型
    pub kind: ToolErrorKind,
    /// 错误信息
    pub message: String,

    /// 产生错误时的参数版本
    pub argument_revision: u32,
    /// 产生错误时使用的参数
    pub arguments: Option<serde_json::Value>,
    /// 产生错误时的执行次数
    pub attempt: u32,
    /// 错误发生时间
    pub occurred_at_ms: u64,
    /// Runtime 对该错误的处理决定
    pub retry_decision: RetryDecision,
}

/// 工具调用的执行信息
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolCallExecution {
    /// 参数的版本号，通过 `set_arguments` 方法修改 `arguments` 的时候会自增
    argument_revision: u32,
    /// 参数
    arguments: Option<serde_json::Value>,
    /// 当前执行状态，表示参数准备、运行、成功或失败等状态
    status: ToolCallStatus,
    /// 当前尝试次数
    attempt: u32,
    /// 下一次重试的时间
    next_retry_at_ms: Option<u64>,
    /// 幂等键
    idempotency_key: Option<String>,
    /// 工具调用的成功结果
    result: Option<ToolSuccess>,
    /// 工具调用的所有错误记录，只追加，不覆盖
    error_records: Vec<ToolErrorRecord>,
}

impl Default for ToolCallExecution {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallExecution {
    /// 创建工具调用的执行信息，执行状态为 `ArgumentsPending`
    #[must_use]
    pub fn new() -> Self {
        Self {
            argument_revision: 0,
            arguments: None,
            status: ToolCallStatus::ArgumentsPending,
            attempt: 0,
            next_retry_at_ms: None,
            idempotency_key: None,
            result: None,
            error_records: Vec::new(),
        }
    }

    /// 获取当前执行状态
    #[must_use]
    pub fn status(&self) -> ToolCallStatus {
        self.status
    }

    /// 设置参数，执行状态变为 `ArgumentsReady`
    pub fn set_arguments(&mut self, arguments: serde_json::Value) {
        self.argument_revision += 1;
        self.arguments = Some(arguments);
        self.status = ToolCallStatus::ArgumentsReady;
        self.attempt = 0;
        self.next_retry_at_ms = None;
        self.idempotency_key = None;
        self.result = None;
    }

    /// 在参数已经设置时标记准备执行，执行状态变为 `ReadyToExecute`。
    ///
    /// 如果尚未设置参数，则保持当前状态不变。
    pub fn mark_ready_to_execute(&mut self) {
        if self.arguments.is_some() {
            self.status = ToolCallStatus::ReadyToExecute;
        }
    }

    /// 标记正在运行，执行状态变为 `Running`
    pub fn mark_running(&mut self) {
        self.attempt += 1;
        self.status = ToolCallStatus::Running;
    }

    /// 记录工具调用成功，执行状态变为 `Succeeded`
    pub fn record_success(&mut self, result: ToolSuccess) {
        self.result = Some(result);
        self.next_retry_at_ms = None;
        self.status = ToolCallStatus::Succeeded;
    }

    /// 记录工具调用失败，执行状态变为：
    ///
    /// * `WaitingRetry` - 决定重试
    /// * `Failed` - 决定不重试
    pub fn record_failure(&mut self, error: ToolError, retry_decision: RetryDecision, now_ms: u64) {
        let ToolError { kind, message } = error;

        self.error_records.push(ToolErrorRecord {
            kind,
            message,
            argument_revision: self.argument_revision,
            arguments: self.arguments.clone(),
            attempt: self.attempt,
            occurred_at_ms: now_ms,
            retry_decision,
        });

        match retry_decision {
            RetryDecision::RetryAfter { delay_ms } => {
                self.next_retry_at_ms = Some(now_ms.saturating_add(delay_ms));
                self.status = ToolCallStatus::WaitingRetry;
            }
            RetryDecision::Stop => {
                self.next_retry_at_ms = None;
                self.status = ToolCallStatus::Failed;
            }
        }
    }
}

/// 执行计划中的单个逻辑调用，包含调用计划和执行信息
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// 工具调用 id
    pub id: ToolCallId,
    /// 执行计划 id
    pub execution_plan_id: ExecutionPlanId,
    /// 工具的调用计划
    pub plan: ToolCallPlan,
    /// 工具的执行信息
    pub execution: ToolCallExecution,
}

impl ToolCall {
    /// 创建一个新的工具调用
    #[must_use]
    pub fn new(id: ToolCallId, execution_plan_id: ExecutionPlanId, plan: ToolCallPlan) -> Self {
        Self {
            id,
            execution_plan_id,
            plan,
            execution: ToolCallExecution::new(),
        }
    }

    /// 设置幂等键，幂等键的格式为 "`{task_id}:{execution_plan_id}:{tool_call_id}:{argument_revision}`"
    pub fn set_idempotency_key(&mut self, task_id: &TaskId) {
        self.execution.idempotency_key = Some(format!(
            "{}:{}:{}:{}",
            task_id, self.execution_plan_id, self.id, self.execution.argument_revision
        ));
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        state::StateDelta,
        task::{
            ExecutionPlanId, TaskId, ToolCallExecution, ToolCallId, ToolCallPlan, ToolCallStatus,
        },
        tool::{RetryDecision, ToolError, ToolErrorKind, ToolSuccess},
    };

    /// 只有终态的工具调用状态才应被识别为已结束
    #[test]
    fn terminal_status_should_be_recognized() {
        let terminal_statuses = [
            ToolCallStatus::Succeeded,
            ToolCallStatus::Failed,
            ToolCallStatus::ExecutionUnknown,
            ToolCallStatus::Invalidated,
        ];

        for status in terminal_statuses {
            assert!(status.is_terminal(), "{status:?} 应该是终态");
        }

        let non_terminal_statuses = [
            ToolCallStatus::ArgumentsPending,
            ToolCallStatus::ArgumentsReady,
            ToolCallStatus::WaitingApproval,
            ToolCallStatus::ReadyToExecute,
            ToolCallStatus::Running,
            ToolCallStatus::WaitingRetry,
        ];

        for status in non_terminal_statuses {
            assert!(!status.is_terminal(), "{status:?} 不应该是终态");
        }
    }

    /// `set_arguments` 应该能达到下面的效果：
    ///
    /// - `argument_revision` 从 `0` 变成 `1`
    /// - 状态变成 `ArgumentsReady`
    /// - `attempt` 被重置为 `0`
    /// - 旧的 `idempotency_key` 和 `result` 被清空
    #[test]
    fn set_arguments_should_reset_execution_state() {
        let mut execution = ToolCallExecution::new();

        execution.attempt = 3;
        execution.idempotency_key = Some("old-key".to_string());
        execution.result = Some(ToolSuccess {
            output: serde_json::json!({"old": true}),
            state_delta: StateDelta::default(),
        });

        execution.set_arguments(serde_json::json!({"limit": 5}));

        assert_eq!(execution.argument_revision, 1);
        assert_eq!(execution.arguments, Some(serde_json::json!({"limit": 5})));
        assert_eq!(execution.status, ToolCallStatus::ArgumentsReady);
        assert_eq!(execution.attempt, 0);
        assert!(execution.idempotency_key.is_none());
        assert!(execution.result.is_none());
    }

    /// `set_idempotency_key` 生成的幂等键应包含任务、执行计划、工具调用和参数版本
    #[test]
    fn set_idempotency_key_should_use_current_identity_and_revision() {
        let mut tool_call = crate::task::ToolCall::new(
            ToolCallId::new("call-1"),
            ExecutionPlanId::new("plan-1"),
            ToolCallPlan {
                call_key: "query".to_string(),
                tool_name: "query_task".to_string(),
                purpose: "查询任务".to_string(),
            },
        );

        tool_call
            .execution
            .set_arguments(serde_json::json!({"limit": 5}));

        tool_call.set_idempotency_key(&TaskId::new("task-1"));

        assert_eq!(
            tool_call.execution.idempotency_key.as_deref(),
            Some("task-1:plan-1:call-1:1")
        );
    }

    /// 可重试错误应记录错误并进入等待重试状态
    #[test]
    fn record_failure_with_retry_should_schedule_next_retry() {
        let mut execution = ToolCallExecution::new();

        execution.set_arguments(serde_json::json!({"limit": 5}));
        execution.mark_ready_to_execute();
        execution.mark_running();

        let error = ToolError {
            kind: ToolErrorKind::Transient,
            message: "临时错误".to_string(),
        };

        execution.record_failure(error, RetryDecision::RetryAfter { delay_ms: 1_000 }, 5_000);

        assert_eq!(execution.status, ToolCallStatus::WaitingRetry);
        assert_eq!(execution.next_retry_at_ms, Some(6_000));
        assert_eq!(execution.error_records.len(), 1);

        let record = &execution.error_records[0];
        assert_eq!(record.kind, ToolErrorKind::Transient);
        assert_eq!(record.argument_revision, 1);
        assert_eq!(record.attempt, 1);
        assert_eq!(record.occurred_at_ms, 5_000);
    }

    /// 不可重试错误应使执行失败，并保留错误记录
    #[test]
    fn record_failure_without_retry_should_mark_failed() {
        let mut execution = ToolCallExecution::new();

        execution.set_arguments(serde_json::json!({"limit": 5}));
        execution.mark_ready_to_execute();
        execution.mark_running();

        let error = ToolError {
            kind: ToolErrorKind::Business,
            message: "业务错误".to_string(),
        };

        execution.record_failure(error, RetryDecision::Stop, 5_000);

        assert_eq!(execution.status, ToolCallStatus::Failed);
        assert!(execution.next_retry_at_ms.is_none());
        assert_eq!(execution.error_records.len(), 1);
        assert_eq!(execution.error_records[0].attempt, 1);
    }

    /// 没有参数时，不应进入待执行状态
    #[test]
    fn mark_ready_without_arguments_should_keep_pending() {
        let mut execution = ToolCallExecution::new();

        execution.mark_ready_to_execute();

        assert_eq!(execution.status(), ToolCallStatus::ArgumentsPending);
    }

    /// 执行成功后，应清除重试时间并标记为成功
    #[test]
    fn record_success_should_mark_succeeded_and_clear_retry_time() {
        let mut execution = ToolCallExecution::new();

        execution.set_arguments(serde_json::json!({"limit": 5}));
        execution.mark_ready_to_execute();
        execution.mark_running();

        execution.record_failure(
            ToolError {
                kind: ToolErrorKind::Transient,
                message: "临时错误".to_string(),
            },
            RetryDecision::RetryAfter { delay_ms: 1_000 },
            5_000,
        );

        execution.record_success(ToolSuccess {
            output: serde_json::json!({"ok": true}),
            state_delta: StateDelta::default(),
        });

        assert_eq!(execution.status, ToolCallStatus::Succeeded);
        assert!(execution.next_retry_at_ms.is_none());
        assert!(execution.result.is_some());
    }
}

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
            ExecutionPlanId, TaskId, ToolCall, ToolCallExecution, ToolCallId, ToolCallPlan,
            ToolCallStatus, ToolErrorRecord,
        },
        tool::{RetryDecision, ToolError, ToolErrorKind, ToolSuccess},
    };

    /// 创建一个 `ToolCallExecution` 对象，并把它的状态变成 [`ToolCallStatus::Running`]。
    fn running_execution() -> ToolCallExecution {
        let mut execution = ToolCallExecution::new();
        execution.set_arguments(serde_json::json!({"limit": 5}));
        execution.mark_ready_to_execute();
        execution.mark_running();
        execution
    }

    /// 创建一个 `ToolCall` 对象。
    fn new_tool_call() -> ToolCall {
        ToolCall::new(
            ToolCallId::new("call-1"),
            ExecutionPlanId::new("plan-1"),
            ToolCallPlan {
                call_key: "query".to_string(),
                tool_name: "query_task".to_string(),
                purpose: "查询任务".to_string(),
            },
        )
    }

    /// 构造一个 [`ToolErrorKind::Transient`] 的临时错误对象。
    fn transient_error() -> ToolError {
        ToolError {
            kind: ToolErrorKind::Transient,
            message: "临时错误".to_string(),
        }
    }

    /// 验证只有终态工具调用状态会被识别为结束。
    ///
    /// 方法：分别遍历终态和非终态集合，检查 `is_terminal` 的结果。
    #[test]
    fn should_recognize_only_terminal_statuses() {
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

    /// 验证设置参数会重置与旧参数相关的执行状态。
    ///
    /// 方法：预先填充旧执行信息后设置新参数，并检查版本、状态和缓存字段。
    #[test]
    fn should_reset_execution_state_when_setting_arguments() {
        let mut execution = ToolCallExecution::new();

        execution.attempt = 3;
        execution.next_retry_at_ms = Some(5_000);
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
        assert_eq!(execution.next_retry_at_ms, None);
        assert!(execution.idempotency_key.is_none());
        assert!(execution.result.is_none());
    }

    /// 验证幂等键包含任务、计划、调用和参数版本身份。
    ///
    /// 方法：为带一版参数的工具调用设置幂等键，并比较完整格式。
    #[test]
    fn should_include_identity_and_argument_revision_in_idempotency_key() {
        let mut tool_call = new_tool_call();

        tool_call
            .execution
            .set_arguments(serde_json::json!({"limit": 5}));

        tool_call.set_idempotency_key(&TaskId::new("task-1"));

        assert_eq!(
            tool_call.execution.idempotency_key.as_deref(),
            Some("task-1:plan-1:call-1:1")
        );
    }

    /// 验证可重试错误会记录完整错误上下文并安排下一次重试。
    ///
    /// 方法：让运行中的调用记录临时错误，并比较状态、重试时间和错误记录快照。
    #[test]
    fn should_schedule_retry_and_record_failure_when_retrying() {
        let mut execution = running_execution();
        let retry_decision = RetryDecision::RetryAfter { delay_ms: 1_000 };

        execution.record_failure(transient_error(), retry_decision, 5_000);

        assert_eq!(execution.status, ToolCallStatus::WaitingRetry);
        assert_eq!(execution.next_retry_at_ms, Some(6_000));
        assert_eq!(
            execution.error_records,
            vec![ToolErrorRecord {
                kind: ToolErrorKind::Transient,
                message: "临时错误".to_string(),
                argument_revision: 1,
                arguments: Some(serde_json::json!({"limit": 5})),
                attempt: 1,
                occurred_at_ms: 5_000,
                retry_decision,
            }]
        );
    }

    /// 验证不可重试错误会标记调用失败且不会安排重试。
    ///
    /// 方法：让运行中的调用记录停止重试的错误，并检查终态与重试时间。
    #[test]
    fn should_mark_failed_without_retry() {
        let mut execution = running_execution();

        let error = ToolError {
            kind: ToolErrorKind::Business,
            message: "业务错误".to_string(),
        };

        execution.record_failure(error, RetryDecision::Stop, 5_000);

        assert_eq!(execution.status, ToolCallStatus::Failed);
        assert!(execution.next_retry_at_ms.is_none());
        // 错误记录的完整字段由可重试分支 `should_schedule_retry_and_record_failure_when_retrying` 覆盖。
    }

    /// 验证没有参数时调用不会进入待执行状态。
    ///
    /// 方法：直接标记新建执行信息为待执行，并检查状态保持不变。
    #[test]
    fn should_remain_arguments_pending_without_arguments() {
        let mut execution = ToolCallExecution::new();

        execution.mark_ready_to_execute();

        assert_eq!(execution.status(), ToolCallStatus::ArgumentsPending);
    }

    /// 验证记录成功会清除重试时间并保存结果。
    ///
    /// 方法：先让调用进入等待重试状态，再记录成功结果并检查终态与结果。
    #[test]
    fn should_mark_succeeded_and_clear_retry_time_when_recording_success() {
        let mut execution = running_execution();

        execution.record_failure(
            transient_error(),
            RetryDecision::RetryAfter { delay_ms: 1_000 },
            5_000,
        );

        let result = ToolSuccess {
            output: serde_json::json!({"ok": true}),
            state_delta: StateDelta::default(),
        };
        execution.record_success(result.clone());

        assert_eq!(execution.status, ToolCallStatus::Succeeded);
        assert!(execution.next_retry_at_ms.is_none());
        assert_eq!(execution.result, Some(result));
    }
}

use crate::task::{ExecutionPlanId, TaskId, ToolCall};

/// 执行计划的状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanStatus {
    /// 执行计划已创建，尚未完成执行前准备
    Planned,
    /// 执行计划中的所有工具调用均已准备完成，可以执行
    Ready,
    /// 执行计划正在执行中
    Running,
    /// 执行计划中的所有工具调用均已成功完成
    Succeeded,
    /// 至少一个工具调用最终失败，执行计划未能完成
    Failed,
    /// 执行计划基于的 `State` 已过期，或其中的 `StateDelta` 存在冲突，需要重新规划
    Conflict,
}

impl ExecutionPlanStatus {
    /// 判断执行计划是否已经结束
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Conflict)
    }
}

/// Runtime 处理的执行计划。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionPlan {
    /// 执行计划 id
    pub id: ExecutionPlanId,
    /// 任务 id
    pub task_id: TaskId,
    /// 任务内执行计划的顺序编号，通常从 `0` 开始
    pub ordinal: u32,
    /// 生成执行计划时所依据的 State 版本号，用于检测计划是否过期
    pub state_version: u64,
    /// 执行计划的状态
    status: ExecutionPlanStatus,
    /// 需要调用的工具集合
    pub tool_calls: Vec<ToolCall>,
}

impl ExecutionPlan {
    /// 创建一个新的执行计划，计划的状态为 `Planned`
    #[must_use]
    pub fn new(
        id: ExecutionPlanId,
        task_id: TaskId,
        ordinal: u32,
        state_version: u64,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            id,
            task_id,
            ordinal,
            state_version,
            status: ExecutionPlanStatus::Planned,
            tool_calls,
        }
    }

    /// 获取执行计划的当前状态
    #[must_use]
    pub fn status(&self) -> ExecutionPlanStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use crate::task::{
        ExecutionPlan, ExecutionPlanId, ExecutionPlanStatus, TaskId, ToolCall, ToolCallId,
        ToolCallPlan,
    };

    /// 只有终态的执行计划状态才应被识别为已结束
    #[test]
    fn terminal_status_should_be_recognized() {
        let terminal_statuses = [
            ExecutionPlanStatus::Succeeded,
            ExecutionPlanStatus::Failed,
            ExecutionPlanStatus::Conflict,
        ];

        for status in terminal_statuses {
            assert!(status.is_terminal(), "{status:?} 应该是终态");
        }

        let non_terminal_statuses = [
            ExecutionPlanStatus::Planned,
            ExecutionPlanStatus::Ready,
            ExecutionPlanStatus::Running,
        ];

        for status in non_terminal_statuses {
            assert!(!status.is_terminal(), "{status:?} 不应该是终态");
        }
    }

    fn build_tool_call(call_id: &str, tool_name: &str) -> ToolCall {
        ToolCall::new(
            ToolCallId::new(call_id),
            ExecutionPlanId::new("plan-1"),
            ToolCallPlan {
                call_key: call_id.to_string(),
                tool_name: tool_name.to_string(),
                purpose: "测试调用".to_string(),
            },
        )
    }

    /// 新建执行计划时，应正确初始化基本字段
    #[test]
    fn new_should_initialize_execution_plan() {
        let plan = ExecutionPlan::new(
            ExecutionPlanId::new("plan-1"),
            TaskId::new("task-1"),
            0,
            3,
            Vec::new(),
        );

        assert_eq!(plan.id, ExecutionPlanId::new("plan-1"));
        assert_eq!(plan.task_id, TaskId::new("task-1"));
        assert_eq!(plan.state_version, 3);
        assert_eq!(plan.status(), ExecutionPlanStatus::Planned);
        assert!(plan.tool_calls.is_empty());
    }

    /// 执行计划的顺序编号应支持从 0 开始
    #[test]
    fn new_should_allow_zero_ordinal() {
        let plan = ExecutionPlan::new(
            ExecutionPlanId::new("plan-1"),
            TaskId::new("task-1"),
            0,
            0,
            Vec::new(),
        );

        assert_eq!(plan.ordinal, 0);
    }

    /// 执行计划应保留创建方传入的顺序编号
    #[test]
    fn new_should_preserve_ordinal() {
        let plan = ExecutionPlan::new(
            ExecutionPlanId::new("plan-1"),
            TaskId::new("task-1"),
            2,
            0,
            Vec::new(),
        );

        assert_eq!(plan.ordinal, 2);
    }

    /// 执行计划应保留工具调用的顺序
    #[test]
    fn new_should_preserve_tool_call_order() {
        let tool_calls = vec![
            build_tool_call("call-1", "tool-a"),
            build_tool_call("call-2", "tool-b"),
        ];

        let plan = ExecutionPlan::new(
            ExecutionPlanId::new("plan-1"),
            TaskId::new("task-1"),
            0,
            0,
            tool_calls,
        );

        assert_eq!(plan.tool_calls[0].plan.tool_name, "tool-a");
        assert_eq!(plan.tool_calls[1].plan.tool_name, "tool-b");
    }

    /// 执行计划应支持 JSON 序列化和反序列化
    #[test]
    fn execution_plan_should_round_trip_through_json() {
        let plan = ExecutionPlan::new(
            ExecutionPlanId::new("plan-1"),
            TaskId::new("task-1"),
            1,
            5,
            vec![build_tool_call("call-1", "tool-a")],
        );

        let json = serde_json::to_string(&plan).expect("执行计划应该可以序列化");
        let restored: ExecutionPlan =
            serde_json::from_str(&json).expect("执行计划应该可以反序列化");

        assert_eq!(restored, plan);
    }
}

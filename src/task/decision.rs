/// LLM 的原始决策，不可直接执行
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// 需要调用工具
    NeedToolCall {
        /// 计划调用的工具集合
        tool_call_plans: Vec<ToolCallPlan>,
    },
    /// 完成
    Finish,
    /// 需要用户输入或修改参数
    NeedUserInput,
    /// 取消
    Abort {
        /// 取消原因
        reason: String,
    },
}

/// LLM 规划的工具调用
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCallPlan {
    /// 执行计划内逻辑调用的稳定标识
    pub call_key: String,
    /// 工具名称
    pub tool_name: String,
    /// 目的
    pub purpose: String,
}

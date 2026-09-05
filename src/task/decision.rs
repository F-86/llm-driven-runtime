/// LLM 的原始决策，不可直接执行
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// 请求 Runtime 规划一个或多个候选工具调用。
    ///
    /// Runtime 必须在持久化前根据工具元数据校验这些调用能否并行执行。
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

/// LLM 规划的单个工具调用
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCallPlan {
    /// 执行计划内逻辑调用的稳定标识，同一执行计划中必须唯一。
    /// 
    /// 便于 LLM/用户 以“主搜索”“备用搜索”这类语义名称引用某次调用
    pub call_key: String,
    /// 工具名称
    pub tool_name: String,
    /// 目的
    pub purpose: String,
}

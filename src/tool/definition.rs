use crate::state::State;

/// 工具
#[async_trait::async_trait]
pub trait Tool {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 工具参数 schema
    fn parameter_schema(&self) -> serde_json::Value;

    /// 执行
    async fn execute(
        &self,
        arguments: serde_json::Value,
        state: &State,
    ) -> Result<ToolResult, ToolError>;
}

/// 工具调用成功的结果
pub struct ToolResult {
    pub output: serde_json::Value,
}

/// 工具调用失败的结果
#[derive(Debug)]
pub struct ToolError {
    pub message: String,
}

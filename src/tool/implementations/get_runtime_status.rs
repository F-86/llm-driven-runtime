use crate::{
    state::State,
    tool::{
        Tool,
        definition::{ToolError, ToolResult},
    },
};

/// 获取 runtime 状态的工具
pub struct GetRuntimeStatus;

#[async_trait::async_trait]
impl Tool for GetRuntimeStatus {
    fn name(&self) -> &str {
        "get_runtime_status"
    }

    fn description(&self) -> &str {
        "获取当前运行时的基本状态"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Value,
        state: &State,
    ) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            output: serde_json::json!({
                "task_id": state.task_id,
                "status": "normal"
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 `get_runtime_status` 工具的 `parameter_schema()` 方法
    #[test]
    fn should_return_empty_parameter_schema() {
        let tool = GetRuntimeStatus;
        let schema = tool.parameter_schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
        assert_eq!(schema["additionalProperties"], false);
    }
}

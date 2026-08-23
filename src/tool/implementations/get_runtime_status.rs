use crate::{
    state::State,
    tool::{
        Tool,
        definition::{ToolError, ToolResult},
    },
};

/// `GetRuntimeStatus` 工具的参数
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetRuntimeStatusArg {}

/// 获取 runtime 状态的工具
pub struct GetRuntimeStatus;

#[tool_macros::tool_schema(GetRuntimeStatusArg)]
#[async_trait::async_trait]
impl Tool for GetRuntimeStatus {
    fn name(&self) -> &str {
        "get_runtime_status"
    }

    fn description(&self) -> &str {
        "获取当前运行时的基本状态"
    }

    async fn execute(&self, arg: String, state: &State) -> Result<ToolResult, ToolError> {
        let _arg = serde_json::from_str::<GetRuntimeStatusArg>(&arg)?;

        Ok(ToolResult {
            output: serde_json::json!({
                "task_id": state.task_id,
                "status": "normal"
            }),
        })
    }
}

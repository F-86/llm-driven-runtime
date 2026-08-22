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
pub struct GetRuntimeStatusParam {}

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
        serde_json::to_value(schemars::schema_for!(GetRuntimeStatusParam))
            .expect("Failed to serialize schema")
    }

    async fn execute(&self, param: String, state: &State) -> Result<ToolResult, ToolError> {
        let _param = serde_json::from_str::<GetRuntimeStatusParam>(&param)?;

        Ok(ToolResult {
            output: serde_json::json!({
                "task_id": state.task_id,
                "status": "normal"
            }),
        })
    }
}

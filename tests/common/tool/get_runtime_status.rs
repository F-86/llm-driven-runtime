use llm_driven_runtime::{
    state::{State, StateDelta},
    tool::{Tool, ToolError, ToolMetadata, ToolSuccess},
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
    fn name(&self) -> &'static str {
        "get_runtime_status"
    }

    fn description(&self) -> &'static str {
        "获取当前运行时的基本状态"
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::read_only(vec![])
    }

    async fn execute(
        &self,
        arg: serde_json::Value,
        _state: &State,
    ) -> Result<ToolSuccess, ToolError> {
        let _arg: GetRuntimeStatusArg = serde_json::from_value(arg)?;

        Ok(ToolSuccess {
            output: serde_json::json!({
                "status": "normal"
            }),
            state_delta: StateDelta { mutations: vec![] },
        })
    }
}

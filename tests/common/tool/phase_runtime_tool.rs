use llm_driven_runtime::{
    state::{State, StateDelta},
    tool::{Tool, ToolError, ToolMetadata, ToolSuccess},
};

/// 仅用于 Phase Runtime 测试的可配置工具。
pub struct MetadataTool {
    name: String,
    metadata: ToolMetadata,
}

impl MetadataTool {
    /// 创建具有给定名称和元数据的测试工具。
    #[must_use]
    pub fn new(name: impl Into<String>, metadata: ToolMetadata) -> Self {
        Self {
            name: name.into(),
            metadata,
        }
    }
}

/// `MetadataTool` 工具的参数
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataToolArg {}

#[tool_macros::tool_schema(MetadataToolArg)]
#[async_trait::async_trait]
impl Tool for MetadataTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &'static str {
        "Phase Runtime 测试工具"
    }

    fn metadata(&self) -> ToolMetadata {
        self.metadata.clone()
    }

    async fn execute(
        &self,
        _arg: serde_json::Value,
        _state: &State,
    ) -> Result<ToolSuccess, ToolError> {
        Ok(ToolSuccess {
            output: serde_json::Value::Null,
            state_delta: StateDelta {
                mutations: Vec::new(),
            },
        })
    }
}

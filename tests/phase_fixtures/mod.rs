use llm_driven_runtime::{
    phase_runtime::{DecisionHandler, DecisionHandlerError},
    state::{State, StateDelta},
    task::{Decision, Task},
    tool::{Tool, ToolError, ToolMetadata, ToolSuccess},
};

/// 始终返回同一个决策的测试替身。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FixedDecisionHandler {
    decision: Decision,
}

impl FixedDecisionHandler {
    /// 创建始终返回给定决策的处理器。
    pub(super) fn new(decision: Decision) -> Self {
        Self { decision }
    }
}

impl DecisionHandler for FixedDecisionHandler {
    fn decide(&self, _task: &Task) -> Result<Decision, DecisionHandlerError> {
        Ok(self.decision.clone())
    }
}

/// 如果被调用就会返回错误的决策处理器，用于验证不应进入决策阶段的路径。
pub(super) struct UnexpectedDecisionHandler;

impl DecisionHandler for UnexpectedDecisionHandler {
    fn decide(&self, _task: &Task) -> Result<Decision, DecisionHandlerError> {
        Err(DecisionHandlerError::new("当前测试路径不应请求决策"))
    }
}

/// `MetadataTool` 工具的参数
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct MetadataToolArg {}

/// 仅用于 Phase Runtime 测试的可配置工具。
pub(super) struct MetadataTool {
    name: String,
    metadata: ToolMetadata,
}

impl MetadataTool {
    /// 创建具有给定名称和元数据的测试工具。
    pub(super) fn new(name: impl Into<String>, metadata: ToolMetadata) -> Self {
        Self {
            name: name.into(),
            metadata,
        }
    }
}

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

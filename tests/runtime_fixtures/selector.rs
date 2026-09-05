use llm_driven_runtime::{
    state::State,
    tool::{SelectionError, ToolSelection, ToolSelector, registry::ToolRegistry},
    user_input::UserInput,
};

/// 始终选择固定工具的 Runtime 测试选择器。
#[derive(Debug, Clone)]
pub(crate) struct FixedToolSelector {
    tool_name: String,
}

impl FixedToolSelector {
    pub(crate) fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
        }
    }
}

#[async_trait::async_trait]
impl ToolSelector for FixedToolSelector {
    async fn select(
        &self,
        _input: &UserInput,
        _state: &State,
        _tools: &ToolRegistry,
    ) -> Result<ToolSelection, SelectionError> {
        Ok(ToolSelection {
            tool_name: self.tool_name.clone(),
        })
    }
}

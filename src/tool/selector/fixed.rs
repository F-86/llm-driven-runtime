use crate::{
    state::State,
    tool::{SelectionError, ToolSelection, ToolSelector, registry::ToolRegistry},
    user_input::UserInput,
};

/// 固定的工具选择器，用于测试
pub struct FixedToolSelector {
    pub tool_name: String,
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

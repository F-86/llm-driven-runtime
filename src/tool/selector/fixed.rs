use crate::{
    state::State,
    tool::{SelectionError, ToolSelection, ToolSelector, registry::ToolRegistry},
    user_input::UserInput,
};

/// 固定的工具选择器，用于测试
pub struct FixedToolSelector<'a> {
    /// 始终返回的工具名称
    pub tool_name: &'a str,
}

#[async_trait::async_trait]
impl ToolSelector for FixedToolSelector<'_> {
    async fn select(
        &self,
        _input: &UserInput,
        _state: &State,
        _tools: &ToolRegistry,
    ) -> Result<ToolSelection, SelectionError> {
        Ok(ToolSelection {
            tool_name: self.tool_name.to_string(),
        })
    }
}

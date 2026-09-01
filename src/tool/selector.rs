use crate::{state::State, tool::registry::ToolRegistry, user_input::UserInput};

/// 工具选择器
#[async_trait::async_trait]
pub trait ToolSelector {
    /// 选择工具
    async fn select(
        &self,
        input: &UserInput,
        state: &State,
        tools: &ToolRegistry,
    ) -> Result<ToolSelection, SelectionError>;
}

/// 工具选择结果
pub struct ToolSelection {
    /// 被选中的工具名称
    pub tool_name: String,
}

/// 工具选择错误
pub struct SelectionError {
    /// 工具选择失败的原因
    pub message: String,
}

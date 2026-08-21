use crate::{state::State, tool::registry::ToolRegistry, user_input::UserInput};

pub mod fixed;

pub use fixed::FixedToolSelector;

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
    pub tool_name: String,
}

/// 工具选择错误
pub struct SelectionError {
    pub message: String,
}

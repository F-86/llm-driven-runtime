use crate::{state::State, tool::Tool, user_input::UserInput};

pub mod empty;

/// 参数生成器
#[async_trait::async_trait]
pub trait ParameterGenerator {
    // 生成参数
    async fn generate(
        &self,
        input: &UserInput,
        state: &State,
        tool: &dyn Tool,
    ) -> Result<serde_json::Value, ParameterGenerationError>;
}

/// 参数生成错误
pub struct ParameterGenerationError {
    pub message: String,
}

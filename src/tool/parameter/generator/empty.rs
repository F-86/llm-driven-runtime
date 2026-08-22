use crate::{
    state::State,
    tool::{
        Tool,
        parameter::generator::{ParameterGenerationError, ParameterGenerator},
    },
    user_input::UserInput,
};

/// 空参数生成器，用于测试
pub struct EmptyParameterGenerator;

#[async_trait::async_trait]
impl ParameterGenerator for EmptyParameterGenerator {
    async fn generate(
        &self,
        _input: &UserInput,
        _state: &State,
        _tool: &dyn Tool,
    ) -> Result<serde_json::Value, ParameterGenerationError> {
        Ok(serde_json::json!({}))
    }
}

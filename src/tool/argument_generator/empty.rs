use crate::{
    state::State,
    tool::{
        Tool,
        argument_generator::{ArgumentGenerationError, ArgumentGenerator},
    },
    user_input::UserInput,
};

/// 空对象参数生成器，用于测试
pub struct EmptyArgumentGenerator;

#[async_trait::async_trait]
impl ArgumentGenerator for EmptyArgumentGenerator {
    async fn generate(
        &self,
        _input: &UserInput,
        _state: &State,
        _tool: &dyn Tool,
    ) -> Result<String, ArgumentGenerationError> {
        Ok("{}".to_string())
    }
}

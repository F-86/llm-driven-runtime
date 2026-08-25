use crate::{
    state::State,
    tool::{
        Tool,
        argument_generator::{ArgumentGenerationError, ArgumentGenerator},
    },
    user_input::UserInput,
};

/// 固定参数生成器，用于测试
pub struct FixedArgumentGenerator<'a> {
    pub arg: &'a str,
}

#[async_trait::async_trait]
impl ArgumentGenerator for FixedArgumentGenerator<'_> {
    async fn generate(
        &self,
        _input: &UserInput,
        _state: &State,
        _tool: &dyn Tool,
    ) -> Result<String, ArgumentGenerationError> {
        Ok(self.arg.to_string())
    }
}

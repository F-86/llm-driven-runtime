use llm_driven_runtime::{
    state::State,
    tool::{
        Tool,
        argument_generator::{ArgumentGenerationError, ArgumentGenerator},
    },
    user_input::UserInput,
};

/// 固定参数生成器，用于测试
pub struct FixedArgumentGenerator {
    /// 始终返回的工具参数
    pub arg: serde_json::Value,
}

#[async_trait::async_trait]
impl ArgumentGenerator for FixedArgumentGenerator {
    async fn generate(
        &self,
        _input: &UserInput,
        _state: &State,
        _tool: &dyn Tool,
    ) -> Result<serde_json::Value, ArgumentGenerationError> {
        Ok(self.arg.clone())
    }
}

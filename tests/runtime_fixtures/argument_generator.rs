use llm_driven_runtime::{
    state::State,
    tool::{
        Tool,
        argument_generator::{ArgumentGenerationError, ArgumentGenerator},
    },
    user_input::UserInput,
};

/// 始终返回固定参数的 Runtime 测试生成器。
#[derive(Debug, Clone)]
pub(crate) struct FixedArgumentGenerator {
    arg: serde_json::Value,
}

impl FixedArgumentGenerator {
    pub(crate) fn new(arg: serde_json::Value) -> Self {
        Self { arg }
    }
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

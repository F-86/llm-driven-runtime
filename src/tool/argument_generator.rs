use crate::{state::State, tool::Tool, user_input::UserInput};

pub mod empty;
pub mod fixed;

pub use empty::EmptyArgumentGenerator;
pub use fixed::FixedArgumentGenerator;

/// 参数生成器
#[async_trait::async_trait]
pub trait ArgumentGenerator {
    // 生成参数
    async fn generate(
        &self,
        input: &UserInput,
        state: &State,
        tool: &dyn Tool,
    ) -> Result<String, ArgumentGenerationError>;
}

/// 参数生成错误
pub struct ArgumentGenerationError {
    pub message: String,
}

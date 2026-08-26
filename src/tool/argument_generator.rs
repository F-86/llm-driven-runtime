use crate::{state::State, tool::Tool, user_input::UserInput};

/// 返回空对象参数的测试实现。
pub mod empty;
/// 返回固定参数的测试实现。
pub mod fixed;

pub use empty::EmptyArgumentGenerator;
pub use fixed::FixedArgumentGenerator;

/// 参数生成器
#[async_trait::async_trait]
pub trait ArgumentGenerator {
    /// 根据用户输入、当前状态和工具契约生成工具参数。
    async fn generate(
        &self,
        input: &UserInput,
        state: &State,
        tool: &dyn Tool,
    ) -> Result<serde_json::Value, ArgumentGenerationError>;
}

/// 参数生成错误
pub struct ArgumentGenerationError {
    /// 参数生成失败的原因
    pub message: String,
}

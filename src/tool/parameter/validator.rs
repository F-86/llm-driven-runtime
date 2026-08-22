use crate::{state::State, tool::Tool};

pub mod empty;

pub use empty::EmptyParameterValidator;

/// 参数校验器
#[async_trait::async_trait]
pub trait ParameterValidator {
    /// 校验参数
    async fn validate(
        &self,
        tool: &dyn Tool,
        params: &serde_json::Value,
        state: &State,
    ) -> Result<(), ParameterValidationError>;
}

/// 参数校验错误
pub struct ParameterValidationError {
    pub message: String,
}

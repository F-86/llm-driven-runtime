use crate::{
    state::State,
    tool::{
        Tool,
        parameter::validator::{ParameterValidationError, ParameterValidator},
    },
};

/// 空参数校验器，用于测试
pub struct EmptyParameterValidator;

#[async_trait::async_trait]
impl ParameterValidator for EmptyParameterValidator {
    async fn validate(
        &self,
        _tool: &dyn Tool,
        params: &serde_json::Value,
        _state: &State,
    ) -> Result<(), ParameterValidationError> {
        match params {
            serde_json::Value::Object(object) if object.is_empty() => Ok(()),
            _ => Err(ParameterValidationError {
                message: "期望空参数".to_string(),
            }),
        }
    }
}

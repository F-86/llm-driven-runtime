use llm_driven_runtime::{
    state::{State, StateDelta},
    tool::{Tool, ToolError, ToolErrorKind, ToolMetadata, ToolSuccess},
};

/// `QueryTask` 工具的参数
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryTaskArg {
    /// 返回数量，默认 10，业务范围为 1 到 100
    #[schemars(range(min = 1, max = 100))]
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

/// 查询任务信息的工具
pub struct QueryTask;

#[tool_macros::tool_schema(QueryTaskArg)]
#[async_trait::async_trait]
impl Tool for QueryTask {
    fn name(&self) -> &'static str {
        "query_task"
    }

    fn description(&self) -> &'static str {
        "查询任务信息"
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::read_only(vec![])
    }

    async fn execute(
        &self,
        arg: serde_json::Value,
        _state: &State,
    ) -> Result<ToolSuccess, ToolError> {
        let arg: QueryTaskArg = serde_json::from_value(arg)?;

        // 手动校验 limit 的业务范围
        if !(1..=100).contains(&arg.limit) {
            return Err(ToolError {
                kind: ToolErrorKind::ArgumentValidation,
                message: format!(
                    "limit 字段的值超出范围，期望：[1, 100]，实际：{}",
                    arg.limit
                ),
            });
        }

        Ok(ToolSuccess {
            output: serde_json::json!({
                "limit": arg.limit,
                "status": "normal",
            }),
            state_delta: StateDelta { mutations: vec![] },
        })
    }
}

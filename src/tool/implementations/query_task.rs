use crate::{
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

#[cfg(test)]
mod tests {
    use crate::{
        state::State,
        tool::{QueryTask, Tool, ToolErrorKind},
    };

    const STATE: &State = &State {
        data: serde_json::Value::Null,
    };

    /// 成功执行，显式传入 `limit`
    ///
    /// 验证正常参数能够：
    ///
    /// - 成功反序列化
    /// - 通过 `limit` 校验
    /// - 返回正确 JSON
    #[tokio::test]
    async fn should_execute_query_task() {
        let result = QueryTask
            .execute(serde_json::json!({"limit": 5}), STATE)
            .await
            .unwrap();

        assert_eq!(result.output["limit"], 5);
        assert_eq!(result.output["status"], "normal");
    }

    /// 成功使用默认值
    ///
    /// 验证不传 `limit` 时，Serde 会使用默认值 `10`
    #[tokio::test]
    async fn should_use_default_limit() {
        let result = QueryTask
            .execute(serde_json::json!({}), STATE)
            .await
            .unwrap();

        assert_eq!(result.output["limit"], 10);
    }

    /// 未知字段失败
    ///
    /// 验证 `deny_unknown_fields` 生效
    #[tokio::test]
    async fn should_reject_unknown_fields() {
        let error = QueryTask
            .execute(serde_json::json!({"unknown": true}), STATE)
            .await
            .unwrap_err();

        assert_eq!(error.kind, ToolErrorKind::ArgumentDeserialization);
        assert!(error.message.contains("unknown field"));
    }

    /// `limit` 范围校验失败，测试下边界之外的值
    ///
    /// `limit` 应该大于等于 `1`，但传入 `0`
    #[tokio::test]
    async fn should_reject_limit_below_minimum() {
        let error = QueryTask
            .execute(serde_json::json!({"limit":0}), STATE)
            .await
            .unwrap_err();

        assert_eq!(error.kind, ToolErrorKind::ArgumentValidation);
        assert_eq!(
            error.message,
            "limit 字段的值超出范围，期望：[1, 100]，实际：0"
        );
    }

    /// `limit` 范围校验失败，测试上边界之外的值
    ///
    /// `limit` 应该小于等于 `100`，但传入 `101`
    #[tokio::test]
    async fn should_reject_limit_above_maximum() {
        let error = QueryTask
            .execute(serde_json::json!({"limit":101}), STATE)
            .await
            .unwrap_err();

        assert_eq!(error.kind, ToolErrorKind::ArgumentValidation);
        assert_eq!(
            error.message,
            "limit 字段的值超出范围，期望：[1, 100]，实际：101"
        );
    }

    /// schema 测试
    ///
    /// 测试 `parameter_schema` 生成的 schema 能否符合下列标准：
    ///
    /// 1. 包含 `limit` 的范围限制
    /// 2. `required` 中不包含 `limit`
    #[test]
    fn should_generate_query_task_schema() {
        let schema = QueryTask.parameter_schema();

        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 100);

        if let Some(required) = schema["required"].as_array() {
            assert!(!required.iter().any(|field| field == "limit"));
        }
    }
}

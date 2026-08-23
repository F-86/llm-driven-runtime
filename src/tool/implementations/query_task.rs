use crate::{
    state::State,
    tool::{
        Tool,
        definition::{ToolError, ToolResult},
    },
};

/// `QueryTask` 工具的参数
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryTaskArg {
    /// 任务 id
    pub task_id: u32,

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
    fn name(&self) -> &str {
        "query_task"
    }

    fn description(&self) -> &str {
        "查询任务信息"
    }

    async fn execute(&self, arg: String, state: &State) -> Result<ToolResult, ToolError> {
        let arg: QueryTaskArg = serde_json::from_str(&arg)?;

        // 校验任务 ID 是否为当前任务
        if arg.task_id != state.task_id {
            return Err(ToolError {
                message: format!(
                    "只能查询当前任务，当前任务 id：{}，传入任务 id：{}",
                    state.task_id, arg.task_id
                ),
            });
        }
        // 手动校验 limit 的业务范围
        if !(1..=100).contains(&arg.limit) {
            return Err(ToolError {
                message: format!(
                    "limit 字段的值超出范围，期望：[1, 100]，实际：{}",
                    arg.limit
                ),
            });
        }

        Ok(ToolResult {
            output: serde_json::json!({
                "task_id": arg.task_id,
                "limit": arg.limit,
                "status": "normal",
            })
            .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        state::State,
        tool::{QueryTask, Tool},
    };

    const STATE: &'static State = &State { task_id: 13 };

    /// 成功执行，显式传入 `limit`
    ///
    /// 验证正常参数能够：
    ///
    /// - 成功反序列化
    /// - 通过 `task_id` 校验
    /// - 通过 `limit` 校验
    /// - 返回正确 JSON
    #[tokio::test]
    async fn should_execute_query_task() {
        let result = QueryTask
            .execute(r#"{"task_id":13,"limit":5}"#.to_string(), &STATE)
            .await
            .unwrap();

        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(output["task_id"], 13);
        assert_eq!(output["limit"], 5);
        assert_eq!(output["status"], "normal");
    }

    /// 成功使用默认值
    ///
    /// 验证不传 `limit` 时，Serde 会使用默认值 `10`
    #[tokio::test]
    async fn should_use_default_limit() {
        let result = QueryTask
            .execute(r#"{"task_id":13}"#.to_string(), &STATE)
            .await
            .unwrap();

        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(output["limit"], 10);
    }

    /// 未知字段失败
    ///
    /// 验证 `deny_unknown_fields` 生效
    #[tokio::test]
    async fn should_reject_unknown_fields() {
        let error = QueryTask
            .execute(r#"{"task_id":13,"unknown":true}"#.to_string(), &STATE)
            .await
            .unwrap_err();

        assert!(error.message.contains("unknown field"));
    }

    /// 非法 JSON 字符串
    #[tokio::test]
    async fn should_reject_invalid_json() {
        let error = QueryTask
            .execute(r#"{"task_id":13,"limit":5"#.to_string(), &STATE)
            .await
            .unwrap_err();

        assert!(!error.message.is_empty());
    }

    /// JSON 错误
    ///
    /// 缺少必填字段 `task_id`
    #[tokio::test]
    async fn should_reject_missing_task_id() {
        let error = QueryTask
            .execute(r#"{"limit":5}"#.to_string(), &STATE)
            .await
            .unwrap_err();

        assert!(error.message.contains("missing field"));
    }

    /// 参数类型错误
    ///
    /// `task_id` 应该是数字，但传入字符串
    #[tokio::test]
    async fn should_reject_invalid_argument_type() {
        let error = QueryTask
            .execute(r#"{"task_id":"13","limit":5}"#.to_string(), &STATE)
            .await
            .unwrap_err();

        assert!(error.message.contains("invalid type"));
    }

    /// `task_id` 业务校验失败
    ///
    /// `task_id` 应该是 `13`，但传入 `99`
    #[tokio::test]
    async fn should_reject_different_task_id() {
        let error = QueryTask
            .execute(r#"{"task_id":99,"limit":5}"#.to_string(), &STATE)
            .await
            .unwrap_err();

        assert_eq!(
            error.message,
            "只能查询当前任务，当前任务 id：13，传入任务 id：99"
        );
    }

    /// `limit` 范围校验失败，测试下边界之外的值
    ///
    /// `limit` 应该大于等于 `1`，但传入 `0`
    #[tokio::test]
    async fn should_reject_limit_below_minimum() {
        let error = QueryTask
            .execute(r#"{"task_id":13,"limit":0}"#.to_string(), &STATE)
            .await
            .unwrap_err();

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
            .execute(r#"{"task_id":13,"limit":101}"#.to_string(), &STATE)
            .await
            .unwrap_err();

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
    /// 2. `required` 中：包含 `task_id`，不包含 `limit`
    #[test]
    fn should_generate_query_task_schema() {
        let schema = QueryTask.parameter_schema();

        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 100);

        let required = schema["required"].as_array().expect("required 应该是数组");

        assert!(required.iter().any(|field| field == "task_id"));
        assert!(!required.iter().any(|field| field == "limit"));
    }
}

use llm_driven_runtime::{
    runtime::{Runtime, RuntimeOutput},
    state::State,
    tool::{Tool, ToolErrorKind},
    user_input::UserInput,
};

use crate::common::{
    argument_generator::FixedArgumentGenerator, selector::FixedToolSelector, tool::QueryTask,
};

mod common;

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

/// 构建 `Runtime` 对象，这个 `Runtime` 使用 `FixedArgumentGenerator`，选择 `QueryTask` 工具
fn build_runtime_with_query_task_selector(
    arg: serde_json::Value,
) -> Runtime<FixedToolSelector<'static>, FixedArgumentGenerator> {
    common::build_runtime(QueryTask.name(), arg)
}

/// 校验 `Runtime` 调用成功时的返回值的正确性
fn assert_success_result(result: &serde_json::Value, limit: u32) {
    assert_eq!(result["limit"], limit);
    assert_eq!(result["status"], "normal");
}

/// 验证 `Runtime` 能成功调用 `QueryTask` 工具
#[tokio::test]
async fn should_execute_query_task_through_runtime() {
    let mut runtime = build_runtime_with_query_task_selector(serde_json::json!({"limit":5}));
    let output = runtime.handle(UserInput::Message(String::new())).await;
    match output {
        RuntimeOutput::Completed { message } => {
            let result: serde_json::Value =
                serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

            assert_success_result(&result, 5);
        }
        RuntimeOutput::Failed { message } => panic!("QueryTask 调用失败：{message}"),
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

/// 验证 `QueryTask` 的默认参数能够通过 `Runtime` 生效
#[tokio::test]
async fn should_execute_query_task_default_through_runtime() {
    let mut runtime = build_runtime_with_query_task_selector(serde_json::json!({}));
    let output = runtime.handle(UserInput::Message(String::new())).await;
    match output {
        RuntimeOutput::Completed { message } => {
            let result: serde_json::Value =
                serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

            assert_success_result(&result, 10);
        }
        RuntimeOutput::Failed { message } => panic!("QueryTask 调用失败：{message}"),
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

/// 验证 `QueryTask` 的参数反序列化错误能够经过 `Runtime` 返回
#[tokio::test]
async fn should_return_failed_when_query_task_arguments_are_invalid() {
    let mut runtime = build_runtime_with_query_task_selector(serde_json::json!({"limit": "5"}));
    let output = runtime.handle(UserInput::Message(String::new())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert!(message.contains("invalid type"));
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

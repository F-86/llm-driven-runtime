use llm_driven_runtime::user_input::UserInput;

use crate::runtime_fixtures::ArgumentEchoToolArg;

mod runtime_fixtures;

/// 以固定选择器和参数生成器运行旧 `Runtime`。
async fn run_runtime(
    tool_name: impl Into<String>,
    arguments: serde_json::Value,
) -> Result<String, String> {
    let mut runtime = runtime_fixtures::build_runtime(tool_name, arguments);

    runtime_fixtures::runtime_message(runtime.handle(UserInput::Message(String::new())).await)
}

/// 验证 Runtime 会拒绝选择器返回的未注册工具。
///
/// 方法：让固定选择器返回不存在的工具名，并检查 Runtime 的失败消息。
#[tokio::test]
async fn should_return_failed_when_selected_tool_does_not_exist() {
    let message = run_runtime("not_exists", serde_json::json!({}))
        .await
        .expect_err("Runtime 应该失败");

    assert_eq!(message, "找不到工具");
}

/// 验证 Runtime 会将生成的参数传给所选工具并返回工具输出。
///
/// 方法：使用参数回显工具和固定参数生成器，检查工具收到的 `limit`。
#[tokio::test]
async fn should_forward_generated_arguments_to_selected_tool() {
    let message = run_runtime(
        runtime_fixtures::ARGUMENT_ECHO_TOOL_NAME,
        serde_json::json!({ "limit": 5 }),
    )
    .await
    .expect("Runtime 应该成功");

    let result: ArgumentEchoToolArg =
        serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

    assert_eq!(result.limit, 5);
}

/// 验证 Runtime 会向调用方传播所选工具的参数反序列化错误。
///
/// 方法：经固定参数生成器传入字符串类型的 `limit`，并检查 Runtime 的失败信息。
#[tokio::test]
async fn should_propagate_selected_tool_argument_deserialization_error() {
    let message = run_runtime(
        runtime_fixtures::ARGUMENT_ECHO_TOOL_NAME,
        serde_json::json!({ "limit": "5" }),
    )
    .await
    .expect_err("Runtime 应该失败");

    assert!(message.contains("invalid type"));
}

use llm_driven_runtime::{
    runtime::{Runtime, RuntimeOutput},
    tool::{
        QueryTask, Tool, argument_generator::FixedArgumentGenerator, selector::FixedToolSelector,
    },
    user_input::UserInput,
};

mod common;

/// 构建 `Runtime` 对象，这个 `Runtime` 使用 `FixedArgumentGenerator`，选择 `QueryTask` 工具
fn build_runtime_with_query_task_selector(
    arg: &'_ str,
) -> Runtime<FixedToolSelector<'_>, FixedArgumentGenerator<'_>> {
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
    let mut runtime = build_runtime_with_query_task_selector(r#"{"limit":5}"#);
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
    let mut runtime = build_runtime_with_query_task_selector(r"{}");
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
    let mut runtime = build_runtime_with_query_task_selector(r#"{"limit":"5"}"#);
    let output = runtime.handle(UserInput::Message(String::new())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert!(message.contains("invalid type"));
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

use llm_driven_runtime::{
    runtime::{Runtime, RuntimeOutput},
    state::State,
    tool::{
        GetRuntimeStatus, QueryTask, Tool, argument_generator::FixedArgumentGenerator,
        registry::ToolRegistry, selector::FixedToolSelector,
    },
    user_input::UserInput,
};

/// 构建 `Runtime` 对象
///
/// `Runtime` 对象中包含两个工具：
///
/// - `GetRuntimeStatus`
/// - `QueryTask`
///
/// # Arguments
///
/// * `tool_name` - 固定选择的工具
/// * `arg` - 固定生成的参数
fn build_runtime<'a, 'b>(
    tool_name: &'a str,
    arg: &'b str,
) -> Runtime<FixedToolSelector<'a>, FixedArgumentGenerator<'b>> {
    let state = State { task_id: 13 };

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(GetRuntimeStatus);
    tool_registry.register(QueryTask);

    let tool_selector = FixedToolSelector {
        tool_name: tool_name,
    };

    let argument_generator = FixedArgumentGenerator { arg };

    Runtime::new(state, tool_registry, tool_selector, argument_generator)
}

/// 构建 `Runtime` 对象，这个 `Runtime` 使用 `FixedArgumentGenerator`，选择 `QueryTask` 工具
///
/// `Runtime` 对象中包含两个工具：
///
/// - `GetRuntimeStatus`
/// - `QueryTask`
///
/// # Arguments
///
/// * `arg` - 固定生成的参数
fn build_runtime_with_query_task_selector(
    arg: &'_ str,
) -> Runtime<FixedToolSelector<'_>, FixedArgumentGenerator<'_>> {
    build_runtime(QueryTask.name(), arg)
}

/// 校验 `Runtime` 调用成功时的返回值的正确性
fn assert_success_result(result: &serde_json::Value, task_id: u32, limit: u32) {
    assert_eq!(result["task_id"], task_id);
    assert_eq!(result["limit"], limit);
    assert_eq!(result["status"], "normal");
}

/// 验证 `Runtime` 能成功调用 `QueryTask` 工具
#[tokio::test]
async fn should_execute_query_task_through_runtime() {
    let mut runtime = build_runtime_with_query_task_selector(r#"{"task_id":13,"limit":5}"#);
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Completed { message } => {
            let result: serde_json::Value =
                serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

            assert_success_result(&result, 13, 5);
        }
        RuntimeOutput::Failed { message } => panic!("QueryTask 调用失败：{message}"),
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

/// 验证 `QueryTask` 的默认参数能够通过 `Runtime` 生效
#[tokio::test]
async fn should_execute_query_task_default_through_runtime() {
    let mut runtime = build_runtime_with_query_task_selector(r#"{"task_id":13}"#);
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Completed { message } => {
            let result: serde_json::Value =
                serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

            assert_success_result(&result, 13, 10);
        }
        RuntimeOutput::Failed { message } => panic!("QueryTask 调用失败：{message}"),
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

/// 验证 `QueryTask` 的业务参数错误能够经过 `Runtime` 返回
#[tokio::test]
async fn should_return_failed_when_querying_another_task() {
    let mut runtime = build_runtime_with_query_task_selector(r#"{"task_id":99,"limit":5}"#);
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert_eq!(
                message,
                "只能查询当前任务，当前任务 id：13，传入任务 id：99"
            );
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

/// 验证 `QueryTask` 的参数反序列化错误能够经过 `Runtime` 返回
#[tokio::test]
async fn should_return_failed_when_query_task_arguments_are_invalid() {
    let mut runtime = build_runtime_with_query_task_selector(r#"{"task_id":"13","limit":5}"#);
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert!(message.contains("invalid type"));
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

/// 验证 `FixedToolSelector` 返回不存在的工具时，Runtime 返回失败
#[tokio::test]
async fn should_return_failed_when_selected_tool_does_not_exist() {
    let mut runtime = build_runtime("not_exists", "");
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert_eq!(message, "找不到工具")
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

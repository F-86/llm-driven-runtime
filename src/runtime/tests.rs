use crate::{
    runtime::{Runtime, RuntimeOutput},
    state::State,
    tool::{
        GetRuntimeStatus, Tool, parameter_generator::empty::EmptyParameterGenerator,
        registry::ToolRegistry, selector::FixedToolSelector,
    },
    user_input::UserInput,
};

/// 验证 `get_runtime_status` 工具在 `Runtime` 中的执行
#[tokio::test]
async fn should_execute_get_runtime_status() {
    let state = State { task_id: 13 };

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(GetRuntimeStatus);

    let tool_selector = FixedToolSelector {
        tool_name: GetRuntimeStatus.name().to_string(),
    };

    let parameter_generator = EmptyParameterGenerator;

    let input = UserInput::Message("".to_string());

    let mut runtime = Runtime::new(state, tool_registry, tool_selector, parameter_generator);
    match runtime.handle(input).await {
        RuntimeOutput::Completed { message } => {
            assert!(message.contains("\"task_id\":13"));
            assert!(message.contains("\"status\":\"normal\""));
        }
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

/// 验证在未找到工具的情况下返回失败信息
#[tokio::test]
async fn should_return_failed_when_tool_not_found() {
    let state = State { task_id: 13 };

    let tool_registry = ToolRegistry::new();

    let tool_selector = FixedToolSelector {
        tool_name: GetRuntimeStatus.name().to_string(),
    };

    let parameter_generator = EmptyParameterGenerator;

    let input = UserInput::Message("".to_string());

    let mut runtime = Runtime::new(state, tool_registry, tool_selector, parameter_generator);
    match runtime.handle(input).await {
        RuntimeOutput::Failed { message } => {
            assert_eq!(message, "找不到工具");
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

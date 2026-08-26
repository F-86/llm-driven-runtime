use llm_driven_runtime::{
    state::State,
    tool::{GetRuntimeStatus, Tool, registry::ToolRegistry},
};

/// 验证 `get_runtime_status` 工具的注册
#[test]
fn should_register_get_runtime_status_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(GetRuntimeStatus);

    let tool = registry.get(GetRuntimeStatus.name());

    assert!(tool.is_some());
}

/// 验证 `get_runtime_status` 工具的执行
#[tokio::test]
async fn should_execute_get_runtime_status_tool() {
    let tool = GetRuntimeStatus;

    let state = State {
        data: serde_json::Value::Null,
    };

    let result = tool.execute(serde_json::json!({}), &state).await.unwrap();

    assert_eq!(result.output["status"], "normal");
}

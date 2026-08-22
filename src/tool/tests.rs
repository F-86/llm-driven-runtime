use crate::{
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

    let state = State { task_id: 42 };

    let result = tool.execute("{}".to_string(), &state).await.unwrap();

    assert_eq!(result.output["task_id"], 42);
    assert_eq!(result.output["status"], "normal");
}

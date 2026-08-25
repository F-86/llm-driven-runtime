use llm_driven_runtime::{
    runtime::RuntimeOutput,
    tool::{GetRuntimeStatus, Tool},
    user_input::UserInput,
};

mod common;

/// 验证 `Runtime` 能成功调用 `GetRuntimeStatus` 工具
#[tokio::test]
async fn should_execute_get_runtime_status_through_runtime() {
    let mut runtime = common::build_runtime(GetRuntimeStatus.name(), "{}");
    let output = runtime.handle(UserInput::Message(String::new())).await;
    match output {
        RuntimeOutput::Completed { message } => {
            let result: serde_json::Value =
                serde_json::from_str(&message).expect("Runtime 应该返回合法 JSON");

            assert_eq!(result["status"], "normal");
        }
        _ => panic!("期望 Runtime 返回 Completed"),
    }
}

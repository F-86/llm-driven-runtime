use llm_driven_runtime::{runtime::RuntimeOutput, user_input::UserInput};

mod common;

/// 验证 `FixedToolSelector` 返回不存在的工具时，Runtime 返回失败
#[tokio::test]
async fn should_return_failed_when_selected_tool_does_not_exist() {
    let mut runtime = common::build_runtime("not_exists", "");
    let output = runtime.handle(UserInput::Message("".to_string())).await;
    match output {
        RuntimeOutput::Failed { message } => {
            assert_eq!(message, "找不到工具")
        }
        _ => panic!("期望 Runtime 返回 Failed"),
    }
}

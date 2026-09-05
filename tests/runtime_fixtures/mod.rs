use llm_driven_runtime::{
    runtime::{Runtime, RuntimeOutput},
    state::{State, StateDelta},
    tool::{Tool, ToolError, ToolMetadata, ToolSuccess, registry::ToolRegistry},
};

mod argument_generator;
mod selector;

use argument_generator::FixedArgumentGenerator;
use selector::FixedToolSelector;

/// 参数回显测试工具的固定名称。
pub(super) const ARGUMENT_ECHO_TOOL_NAME: &str = "argument_echo";

/// `ArgumentEchoTool` 工具的参数。
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ArgumentEchoToolArg {
    pub(super) limit: u32,
}

/// 仅用于验证 Runtime 参数传递和错误传播的工具。
struct ArgumentEchoTool;

#[tool_macros::tool_schema(ArgumentEchoToolArg)]
#[async_trait::async_trait]
impl Tool for ArgumentEchoTool {
    fn name(&self) -> &'static str {
        ARGUMENT_ECHO_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "回显参数以验证 Runtime 调用链路"
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::read_only(vec![])
    }

    async fn execute(
        &self,
        arg: serde_json::Value,
        _state: &State,
    ) -> Result<ToolSuccess, ToolError> {
        let arguments: ArgumentEchoToolArg = serde_json::from_value(arg)?;

        Ok(ToolSuccess {
            output: serde_json::json!({ "limit": arguments.limit }),
            state_delta: StateDelta { mutations: vec![] },
        })
    }
}

/// 构建包含参数回显工具的旧 Runtime 测试实例。
pub(super) fn build_runtime(
    tool_name: impl Into<String>,
    arg: serde_json::Value,
) -> Runtime<FixedToolSelector, FixedArgumentGenerator> {
    let state = State {
        data: serde_json::Value::Null,
    };
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(ArgumentEchoTool);

    Runtime::new(
        state,
        tool_registry,
        FixedToolSelector::new(tool_name),
        FixedArgumentGenerator::new(arg),
    )
}

/// 提取 Runtime 的成功或失败消息；非终态输出会使测试失败。
pub(super) fn runtime_message(output: RuntimeOutput) -> Result<String, String> {
    match output {
        RuntimeOutput::Completed { message } => Ok(message),
        RuntimeOutput::Failed { message } => Err(message),
        _ => panic!("期望 Runtime 返回终态输出"),
    }
}

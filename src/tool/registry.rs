use std::collections::HashMap;

use crate::tool::Tool;

/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// 创建一个空的工具注册表。
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册工具
    pub fn register<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    /// 获取工具
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::ToolRegistry;
    use crate::{
        state::{State, StateDelta},
        tool::{Tool, ToolError, ToolMetadata, ToolSuccess},
    };

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct TestToolArg;

    /// 仅用于验证注册表行为的最小工具实现。
    struct TestTool;

    #[tool_macros::tool_schema(TestToolArg)]
    #[async_trait::async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &'static str {
            "test_tool"
        }

        fn description(&self) -> &'static str {
            "测试工具"
        }

        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::read_only(vec![])
        }

        async fn execute(
            &self,
            _arg: serde_json::Value,
            _state: &State,
        ) -> Result<ToolSuccess, ToolError> {
            Ok(ToolSuccess {
                output: serde_json::Value::Null,
                state_delta: StateDelta { mutations: vec![] },
            })
        }
    }

    /// 验证注册后的工具可以按名称读取。
    ///
    /// 方法：注册一个最小测试工具，再按其名称读取并比较名称。
    #[test]
    fn should_return_registered_tool_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool);

        assert_eq!(registry.get("test_tool").map(Tool::name), Some("test_tool"));
    }
}

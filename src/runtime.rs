use crate::{
    state::State,
    tool::{ToolSelector, parameter_generator::ParameterGenerator, registry::ToolRegistry},
    user_input::UserInput,
};

#[cfg(test)]
mod tests;

/// 运行时
pub struct Runtime<S, G> {
    /// 状态
    state: State,
    /// 工具注册表
    tool_registry: ToolRegistry,
    /// 工具选择器
    tool_selector: S,
    /// 参数生成器
    parameter_generator: G,
}

impl<S, G> Runtime<S, G>
where
    S: ToolSelector,
    G: ParameterGenerator,
{
    pub fn new(
        state: State,
        tool_registry: ToolRegistry,
        tool_selector: S,
        parameter_generator: G,
    ) -> Self {
        Self {
            state,
            tool_registry,
            tool_selector,
            parameter_generator,
        }
    }

    /// 处理
    pub async fn handle(&mut self, input: UserInput) -> RuntimeOutput {
        // 选择工具
        let selection = match self
            .tool_selector
            .select(&input, &self.state, &self.tool_registry)
            .await
        {
            Ok(selection) => selection,
            Err(error) => {
                return RuntimeOutput::Failed {
                    message: error.message,
                };
            }
        };

        // 获取工具
        let Some(tool) = self.tool_registry.get(&selection.tool_name) else {
            return RuntimeOutput::Failed {
                message: "找不到工具".to_string(),
            };
        };

        // 生成参数
        let params = match self
            .parameter_generator
            .generate(&input, &self.state, tool)
            .await
        {
            Ok(res) => res,
            Err(error) => {
                return RuntimeOutput::Failed {
                    message: error.message,
                };
            }
        };

        // 调用工具
        match tool.execute(params, &self.state).await {
            Ok(result) => RuntimeOutput::Completed {
                message: result.output.to_string(),
            },
            Err(error) => RuntimeOutput::Failed {
                message: error.message,
            },
        }
    }
}

pub enum RuntimeOutput {
    Completed { message: String },
    Failed { message: String },
    NeedConfirmation { message: String },
    NeedUserInput { message: String },
}

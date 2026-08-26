use crate::{
    state::State,
    tool::{ToolSelector, argument_generator::ArgumentGenerator, registry::ToolRegistry},
    user_input::UserInput,
};

/// 运行时
pub struct Runtime<S, G> {
    /// 状态
    state: State,
    /// 工具注册表
    tool_registry: ToolRegistry,
    /// 工具选择器
    tool_selector: S,
    /// 参数生成器
    argument_generator: G,
}

impl<S, G> Runtime<S, G>
where
    S: ToolSelector,
    G: ArgumentGenerator,
{
    /// 创建一个运行时，使用给定的状态、工具注册表、工具选择器和参数生成器。
    pub fn new(
        state: State,
        tool_registry: ToolRegistry,
        tool_selector: S,
        argument_generator: G,
    ) -> Self {
        Self {
            state,
            tool_registry,
            tool_selector,
            argument_generator,
        }
    }

    /// 处理一条用户输入，依次完成工具选择、参数生成和工具执行。
    ///
    /// 当前原型不会推进持久化 `TaskPhase`，只返回本次单工具调用的结果。
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
        let arg = match self
            .argument_generator
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
        match tool.execute(arg, &self.state).await {
            Ok(result) => RuntimeOutput::Completed {
                message: result.output.to_string(),
            },
            Err(error) => RuntimeOutput::Failed {
                message: error.message,
            },
        }
    }
}

/// 当前单工具原型返回给用户的结果
pub enum RuntimeOutput {
    /// 运行成功
    Completed {
        /// 返回给用户的结果消息
        message: String,
    },
    /// 运行失败
    Failed {
        /// 返回给用户的失败原因
        message: String,
    },
    /// 需要用户确认；当前 `Runtime::handle` 尚未返回此结果。
    NeedConfirmation {
        /// 需要用户确认的内容
        message: String,
    },
    /// 需要用户输入；当前 `Runtime::handle` 尚未返回此结果。
    NeedUserInput {
        /// 需要用户补充的内容
        message: String,
    },
}

/// 目标持久化运行时处理一个 Phase 后返回的结果。
///
/// 当前单工具原型的 `Runtime::handle` 返回 `RuntimeOutput`，尚未使用此枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
    /// 当前 Phase 已成功处理，并推进到了下一个 Phase
    PhaseAdvanced,
    /// 当前 Phase 暂时无法完成，任务将延后再次调度
    Deferred,
    /// 任务正在等待用户输入或审批，暂不自动调度
    Suspended,
    /// 任务已经成功完成
    Completed,
    /// 任务处理失败并进入失败终态
    Failed,
    /// 调度消息已经过期或重复，不再处理
    Stale,
}

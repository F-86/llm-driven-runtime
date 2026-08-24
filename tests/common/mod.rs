use llm_driven_runtime::{
    runtime::Runtime,
    state::State,
    tool::{
        GetRuntimeStatus, QueryTask, argument_generator::FixedArgumentGenerator,
        registry::ToolRegistry, selector::FixedToolSelector,
    },
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
pub fn build_runtime<'a, 'b>(
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

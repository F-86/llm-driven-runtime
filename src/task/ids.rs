use std::fmt;

/// 定义 id 结构体的宏
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        #[doc = concat!(stringify!($name), " 的稳定标识。")]
        pub struct $name(#[doc = "标识文本"] pub String);

        impl $name {
            /// 从字符串创建标识。
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

// 任务 id
define_id!(TaskId);
// 执行计划 id
define_id!(ExecutionPlanId);
// 工具调用 id
define_id!(ToolCallId);

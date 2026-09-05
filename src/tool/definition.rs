use crate::state::{State, StateDelta};

/// 工具副作用类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    /// 只读，不产生写副作用；是否需要审批由工具元数据决定
    ReadOnly,
    /// 会产生写副作用；是否需要用户审批由工具元数据决定
    Write,
}

/// 工具资源模式
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourcePattern(
    /// 资源匹配模式
    pub String,
);

/// 工具元数据
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolMetadata {
    /// 副作用类型
    pub side_effect: SideEffect,
    /// 是否需要用户审批
    pub requires_approval: bool,
    /// 声明需要读的资源
    pub read_resources: Vec<ResourcePattern>,
    /// 声明需要写的资源
    pub write_resources: Vec<ResourcePattern>,
}

impl ToolMetadata {
    /// 构建一个只读工具的元数据。
    #[must_use]
    pub fn read_only(read_resources: Vec<ResourcePattern>) -> Self {
        Self {
            side_effect: SideEffect::ReadOnly,
            requires_approval: false,
            read_resources,
            write_resources: Vec::new(),
        }
    }
}

/// 工具成功结果
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolSuccess {
    /// 输出
    pub output: serde_json::Value,
    /// 状态的增量
    pub state_delta: StateDelta,
}

/// 工具错误枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorKind {
    /// 参数反序列化错误
    ArgumentDeserialization,
    /// 参数校验错误
    ArgumentValidation,
    /// 当前调用主体没有权限执行该工具
    PermissionDenied,
    /// 业务错误
    Business,
    /// 临时性错误，稍后重试可能成功
    Transient,
    /// 被限流的错误
    RateLimited,
    /// 工具是否实际执行成功无法确认
    ExecutionUnknown,
    /// 系统错误
    System,
}

/// 用户审批枚举
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// 通过
    Approved,
    /// 拒绝
    Rejected {
        /// 可选的拒绝原因
        reason: Option<String>,
    },
}

/// 工具执行的错误
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolError {
    /// 错误类型
    pub kind: ToolErrorKind,
    /// 错误信息
    pub message: String,
}

impl From<serde_json::Error> for ToolError {
    fn from(value: serde_json::Error) -> Self {
        ToolError {
            kind: ToolErrorKind::ArgumentDeserialization,
            message: value.to_string(),
        }
    }
}

/// 工具重试决策
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDecision {
    /// 延迟后重试
    RetryAfter {
        /// 重试延迟，单位为毫秒
        delay_ms: u64,
    },
    /// 停止
    Stop,
}

/// 工具重试策略
pub trait ToolRetryPolicy: Send + Sync {
    /// 获取工具的重试策略
    fn retry_decision(&self, error: &ToolError, attempt: u32) -> RetryDecision;
}

/// 工具契约
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 工具参数 schema
    fn parameter_schema(&self) -> serde_json::Value;

    /// 工具元数据
    fn metadata(&self) -> ToolMetadata;

    /// 执行
    async fn execute(
        &self,
        arg: serde_json::Value,
        state: &State,
    ) -> Result<ToolSuccess, ToolError>;
}

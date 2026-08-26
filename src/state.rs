/// 任务当前状态快照
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// 任务当前持有的结构化状态数据
    pub data: serde_json::Value,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

/// 工具执行后建议应用到 State 的变更集合
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateDelta {
    /// 按顺序应用到 `State` 的变更操作
    pub mutations: Vec<StateMutation>,
}

impl Default for StateDelta {
    fn default() -> Self {
        Self::new()
    }
}

impl StateDelta {
    /// 创建一个不包含任何变更的状态增量。
    #[must_use]
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
        }
    }

    /// 变更是否为空
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }
}

/// 受限的 State 变更操作
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum StateMutation {
    /// 设置指定路径对应的值
    Set {
        /// 要修改的状态路径
        path: Vec<String>,
        /// 要写入路径的值
        value: serde_json::Value,
    },
    /// 删除指定路径对应的值
    Remove {
        /// 要删除的状态路径
        path: Vec<String>,
    },
}

/// State 合并器
pub trait StateReducer {
    /// 把 State 和变更合并成一个 State 并返回
    ///
    /// # Errors
    ///
    /// 如果状态无法合并，则会返回 `StateReduceError` 错误
    fn reduce(&self, state: &State, deltas: &[StateDelta]) -> Result<State, StateReduceError>;
}

/// State 合并错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateReduceError {
    /// `StateMutation` 的路径无效，无法应用变更
    InvalidPath {
        /// 无效的状态路径
        path: Vec<String>,
    },
    /// 多个 `StateMutation` 对同一路径产生了无法合并的冲突
    Conflict {
        /// 发生冲突的状态路径
        path: Vec<String>,
    },
    /// `StateReducer` 不支持该变更操作
    UnsupportedMutation {
        /// 不支持的变更操作名称
        operation: String,
    },
}

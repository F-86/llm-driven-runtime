/// 调度状态枚举，表示“当前能不能调度”
pub enum SchedulingStatus {
    /// 可以被 Dispatcher 调度
    Ready,

    /// 已经放入内存队列
    Queued,

    /// Worker 正在处理
    Running,

    /// 等待外部输入，不允许自动调度
    Suspended,

    /// Task 已经结束
    Terminal,
}

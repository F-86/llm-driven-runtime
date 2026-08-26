/// 用户提交给当前原型运行时的输入。
pub enum UserInput {
    /// 用户发送的文本消息。
    Message(String),
    /// 用户确认上一项请求。
    Confirm,
    /// 用户拒绝上一项请求。
    Reject,
}

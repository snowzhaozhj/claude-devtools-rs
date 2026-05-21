//! Provider 类型标识。
//!
//! 决定 caller 是否需要走"远端友好性"约束（避免 N 次串行 stat 等）。
//! SSH 模式下 caller 的优化决策（如批量、预取）应基于 [`FsKind::Ssh`] 判断。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FsKind {
    Local,
    Ssh,
}

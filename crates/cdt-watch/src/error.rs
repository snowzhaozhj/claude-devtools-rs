//! `cdt-watch` 的错误类型。

/// 文件监听器错误。
#[derive(thiserror::Error, Debug)]
pub enum WatchError {
    /// 初始化底层 watcher 失败。
    #[error("watcher init failed: {0}")]
    Init(#[from] notify::Error),
}

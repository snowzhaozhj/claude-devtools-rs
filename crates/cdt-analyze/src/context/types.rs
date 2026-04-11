//! 参数辅助类型 —— 避免函数签名里展开一堆 `HashMap`。

use std::collections::HashMap;
use std::path::Path;

use cdt_core::{ClaudeMdFileInfo, MentionedFileInfo};

/// 外部注入的 token 数据字典。
///
/// `claude_md` / `directory` / `mentioned_file` 三个字典的 key 是规范化
/// 后的文件路径（绝对路径字符串）。查不到 key 时 aggregator 按 `0 token`
/// 处理，不会报错 —— 对齐 spec 的 "missing token data falls back to zero"。
#[derive(Debug, Clone, Copy)]
pub struct TokenDictionaries<'a> {
    pub project_root: &'a Path,
    pub claude_md: &'a HashMap<String, ClaudeMdFileInfo>,
    pub directory: &'a HashMap<String, ClaudeMdFileInfo>,
    pub mentioned_file: &'a HashMap<String, MentionedFileInfo>,
}

impl<'a> TokenDictionaries<'a> {
    #[must_use]
    pub fn new(
        project_root: &'a Path,
        claude_md: &'a HashMap<String, ClaudeMdFileInfo>,
        directory: &'a HashMap<String, ClaudeMdFileInfo>,
        mentioned_file: &'a HashMap<String, MentionedFileInfo>,
    ) -> Self {
        Self {
            project_root,
            claude_md,
            directory,
            mentioned_file,
        }
    }
}

//! 不变性测试：拦截"未来加新的 `fs.kind() == Ssh` / `let is_remote =` 业务分叉"。
//!
//! change `backend-policy-struct` design D6 + tasks 6.x。`BackendPolicy` /
//! `BackendResolvers` 把 `LocalDataApi` 业务路径的 backend-specific 行为上移到
//! struct 字段后，业务 callsite SHALL 通过 `policy.<field>` / `resolvers.<field>`
//! 读取，**禁止**直接 `match fs.kind()`。本测试扫 `crates/cdt-api/src/ipc/local.rs`
//! 单文件，统计两种 pattern 出现次数并断言 ≤ 真实剩余阈值。
//!
//! ## PR-E 完成后允许出现的位置
//!
//! - **line 812** `list_sessions_skeleton` SSH page cache lookup 派生
//!   （PR-D `unify-fs-direct-calls` 落地的 SSH-Local 同入口下的内部派生，
//!   不在 PR-E 6 处之列）
//! - **line 1601** `build_group_session_page` 同理
//! - **line 3133** `read_mentioned_file` SSH gate（codex design 二审 Open
//!   Question；未来 PR-G 加 `BackendPolicy::supports_mention_file_resolution: bool`
//!   字段后消除）
//!
//! ## 调阈值规则
//!
//! 新增 `fs.kind() == ` / `let is_remote = ` 出现行的 PR SHALL 在 PR 描述中：
//! 1. 引用 design D6
//! 2. 列出新增 callsite 的行号与合理性
//! 3. 引用对应 spec scenario 或 followups.md 条目证明该 fork 应保留
//!
//! 减少出现行（如 PR-G 消除 3133 后）也 SHALL 同步调低阈值。

use std::path::PathBuf;

const MAX_LET_IS_REMOTE: usize = 2;
const MAX_FS_KIND_EQ: usize = 3;

#[test]
fn local_rs_has_at_most_two_let_is_remote_bindings() {
    let path = local_rs_path();
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let count = count_occurrences(&src, "let is_remote =");
    assert!(
        count <= MAX_LET_IS_REMOTE,
        "`let is_remote =` 出现 {count} 次，超过阈值 {MAX_LET_IS_REMOTE}。\
         新增 fork = 违反 fs-abstraction spec scenario \"业务代码通过 BackendPolicy 字段选择行为\"；\
         若需调阈值，按本测试顶部 docstring 的步骤更新 + PR 描述写明合理性。"
    );
}

#[test]
fn local_rs_has_at_most_three_fs_kind_eq_comparisons() {
    let path = local_rs_path();
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let count = count_occurrences(&src, "fs.kind() ==");
    assert!(
        count <= MAX_FS_KIND_EQ,
        "`fs.kind() ==` 出现 {count} 次，超过阈值 {MAX_FS_KIND_EQ}。\
         业务 callsite SHALL 通过 BackendPolicy / BackendResolvers 字段表达后端策略；\
         `fs.kind()` 比对仅允许在 active_fs_and_policy / BackendResolvers::from_fs 派生点使用。\
         若需调阈值，按本测试顶部 docstring 的步骤更新 + PR 描述写明合理性。"
    );
}

fn local_rs_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ipc/local.rs")
}

fn count_occurrences(src: &str, needle: &str) -> usize {
    src.matches(needle).count()
}

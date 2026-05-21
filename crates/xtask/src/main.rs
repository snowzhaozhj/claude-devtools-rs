//! `cargo xtask` —— 仓库自动化任务入口。
//!
//! 子命令：
//! - `check-fs-direct-calls [--warn-only]` —— 扫业务 crate 内是否有 `tokio::fs::*`
//!   直调（H1 Allowlist 真相源：`.claude/rules/fs-abstraction.md`）。
//!   `--warn-only` 命中报 warning 但 exit 0（本 change PR-A 期间默认开启）；
//!   不带 flag 时命中 exit 1（PR-D 完成 callsite 迁移后切到 fail-on-match）。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod check_fs_direct_calls;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next();
    let rest: Vec<String> = args.collect();
    if cmd.as_deref() == Some("check-fs-direct-calls") {
        check_fs_direct_calls::run(&workspace_root(), &rest)
    } else {
        eprintln!("usage: cargo xtask <subcommand>");
        eprintln!("subcommands:");
        eprintln!("  check-fs-direct-calls [--warn-only]");
        ExitCode::from(2)
    }
}

/// 推断 workspace 根：从 binary 当前可执行文件所在路径回溯到 `Cargo.toml` 含
/// `[workspace]` section 的目录。`cargo xtask` 通过 alias 启动时 `current_dir`
/// 由用户决定，避免依赖。
fn workspace_root() -> PathBuf {
    let mut dir: PathBuf = std::env::current_dir().expect("current_dir");
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.exists()
            && std::fs::read_to_string(&manifest)
                .ok()
                .is_some_and(|s| s.contains("[workspace]"))
        {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!(
        "could not locate workspace root from current dir; run cargo xtask from inside the repo"
    );
}

/// 路径斜杠归一化 —— Windows 上 `\` 转 `/`，方便 allowlist 表达式一致。
pub(crate) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

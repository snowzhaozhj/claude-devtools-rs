#!/usr/bin/env bash
# PostToolUse hook: 改 `crates/cdt-*/src/lib.rs` 的 pub API 后，自动失效
# src-tauri 独立 manifest 的 target cache —— 避免 "no xxx in the root" 类
# 陷阱（workspace clippy 能过但 `cargo clippy --manifest-path src-tauri/...`
# 报错，根因 src-tauri/target/ 独立缓存未感知 workspace API 变化）。
#
# 约定：
# - 只对 `crates/cdt-*/src/lib.rs` 触发（pub use 最常在此）。其他 .rs 改
#   cargo 的 incremental 足以处理。
# - 只 `cargo clean -p <crate> --manifest-path src-tauri/Cargo.toml`，不清
#   workspace target —— 精准失效，下次 src-tauri 编译时只重编该 crate。
# - 静默成功，失败也只 warn 不 block（不是致命错，用户可以自行重 build）。
set -euo pipefail

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

if [[ -z "$file_path" ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

# 只匹配 crates/cdt-*/src/lib.rs
if [[ "$rel" != crates/cdt-*/src/lib.rs ]]; then
  exit 0
fi

crate=$(echo "$rel" | awk -F/ '{print $2}')
if [[ -z "$crate" ]]; then
  exit 0
fi

cd "$project_dir"

# src-tauri 的 Cargo.toml 不存在 / 还未引用该 crate 就跳过（不是错）
if [[ ! -f src-tauri/Cargo.toml ]]; then
  exit 0
fi
if ! grep -q "^${crate} *=" src-tauri/Cargo.toml; then
  exit 0
fi

# 静默 clean；若失败只 warn
if ! output=$(cargo clean -p "$crate" --manifest-path src-tauri/Cargo.toml 2>&1); then
  {
    echo "warn: failed to invalidate src-tauri cache for crate '$crate':"
    echo "$output" | tail -5
  } >&2
  exit 0
fi

exit 0

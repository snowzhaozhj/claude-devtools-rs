#!/usr/bin/env bash
# PostToolUse hook: 改 crates/cdt-*/src/lib.rs 的 pub API 后失效 src-tauri 独立 manifest 的 target cache。
#
# 性能预算：99% 命中（非 lib.rs 编辑）case 预判 exit 0，~5ms
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：不含 lib.rs 直接放行
case "$input" in
  *'lib.rs"'*) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

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

if [[ ! -f src-tauri/Cargo.toml ]]; then
  exit 0
fi
if ! grep -q "^${crate} *=" src-tauri/Cargo.toml; then
  exit 0
fi

if ! output=$(cargo clean -p "$crate" --manifest-path src-tauri/Cargo.toml 2>&1); then
  {
    echo "warn: failed to invalidate src-tauri cache for crate '$crate':"
    echo "$output" | tail -5
  } >&2
  exit 0
fi

exit 0

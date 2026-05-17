#!/usr/bin/env bash
# PostToolUse hook: 编辑 .rs 文件后自动 cargo fmt 格式化该文件。
#
# 性能预算：99% 命中（非 .rs 编辑）case 预判 exit 0，~5ms
set -euo pipefail

input=$(</dev/stdin)

# 快速预判
case "$input" in
  *'.rs"'*) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ -z "$file_path" || "$file_path" != *.rs ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

# 触发：crates/ 下的 workspace crate 或 src-tauri/ 下的独立 manifest
# （src-tauri 不在 workspace，但 cargo fmt 单文件粒度跨 manifest 都工作；
# 历史上 src-tauri 的 .rs 编辑没自动 fmt 是 PR #139 反思的根因之一）
if [[ "$rel" != crates/* && "$rel" != src-tauri/* ]]; then
  exit 0
fi

cd "$project_dir"
cargo fmt -- "$file_path" 2>/dev/null || true

exit 0

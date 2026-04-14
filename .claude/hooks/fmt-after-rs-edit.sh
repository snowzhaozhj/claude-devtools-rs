#!/usr/bin/env bash
# PostToolUse hook: 编辑 .rs 文件后自动 cargo fmt 格式化该文件，
# 避免 CI 因格式问题失败。
set -euo pipefail

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

if [[ -z "$file_path" || "$file_path" != *.rs ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

# 只对 crates/ 下的 .rs 文件触发
if [[ "$rel" != crates/* ]]; then
  exit 0
fi

cd "$project_dir"
cargo fmt -- "$file_path" 2>/dev/null || true

exit 0

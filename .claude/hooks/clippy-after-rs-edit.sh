#!/usr/bin/env bash
# PostToolUse hook: 在编辑/写入 .rs 文件后对所属 crate 跑 clippy，
# 让 doc_markdown、needless_pass_by_value 等 pedantic 违规当场暴露，
# 而不是等任务声称完成时才发现。
#
# 约定：只对 crates/<crate>/** 下的 .rs 文件触发；workspace root 或 tests
# fixtures 不触发，避免对不相关编辑做全量 clippy。
set -euo pipefail

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

if [[ -z "$file_path" || "$file_path" != *.rs ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

if [[ "$rel" != crates/* ]]; then
  exit 0
fi

crate=$(echo "$rel" | awk -F/ '{print $2}')
if [[ -z "$crate" ]]; then
  exit 0
fi

cd "$project_dir"
if ! output=$(cargo clippy -p "$crate" --all-targets -- -D warnings 2>&1); then
  {
    echo "clippy failed for crate '$crate' after editing $rel:"
    echo "$output" | tail -60
  } >&2
  exit 2
fi

exit 0

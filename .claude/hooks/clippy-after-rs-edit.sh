#!/usr/bin/env bash
# PostToolUse hook: 编辑 .rs 文件后对所属 crate 跑 clippy，
# 让 doc_markdown / needless_pass_by_value 等 pedantic 违规当场暴露。
#
# 约定：只对 crates/<crate>/** 下的 .rs 文件触发。
#
# 性能预算（见 .claude/rules/perf.md "Hook 性能" 段）：
# - 99% 命中（编辑 .ts / .svelte / .md 等）：case 预判 exit 0，~5ms
# - 编辑 .rs 但不在 crates/：jq 提取后 exit 0，~30ms
# - 编辑 crates/*.rs：跑 cargo clippy（业务必要开销）
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：input 不含 .rs 后缀的 file_path 直接放行
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

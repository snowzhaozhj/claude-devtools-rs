#!/usr/bin/env bash
# PostToolUse hook: 编辑 ui/ 下的 .svelte / .ts 文件后跑 svelte-check 让类型错误当场暴露。
#
# 性能预算：99% 命中（非 .svelte / .ts 编辑）case 预判 exit 0，~5ms
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：不含 .svelte 或 .ts 后缀直接放行
case "$input" in
  *'.svelte"'*|*'.ts"'*) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ -z "$file_path" ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

# 只对 ui/ 下的 .svelte / .ts 文件触发
if [[ "$rel" != ui/* ]]; then
  exit 0
fi

case "$rel" in
  *.svelte|*.ts) ;;
  *) exit 0 ;;
esac

cd "$project_dir/ui"
if ! output=$(npx svelte-check --tsconfig ./tsconfig.app.json 2>&1); then
  {
    echo "svelte-check failed after editing $rel:"
    echo "$output" | tail -30
  } >&2
  exit 2
fi

exit 0

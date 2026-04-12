#!/usr/bin/env bash
# PostToolUse hook: 在编辑/写入 ui/ 下的 .svelte/.ts 文件后跑 svelte-check，
# 让类型错误当场暴露。
set -euo pipefail

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

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

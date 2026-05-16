#!/usr/bin/env bash
# PostToolUse hook: 编辑 tests/fixtures/*.jsonl 后逐行校验 JSON 合法性。
#
# 性能预算：99% 命中（非 .jsonl 编辑）case 预判 exit 0，~5ms
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：不含 .jsonl 后缀直接放行
case "$input" in
  *'.jsonl"'*) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ -z "$file_path" || "$file_path" != *.jsonl ]]; then
  exit 0
fi
if [[ "$file_path" != *tests/fixtures/* ]]; then
  exit 0
fi
if [[ ! -f "$file_path" ]]; then
  exit 0
fi

# jq 逐行解析 JSONL（比 python3 快 ~2.5×）；输出错误行号
errors=$(jq -c -e . "$file_path" 2>&1 1>/dev/null || true)
if [[ -n "$errors" ]]; then
  {
    echo "invalid JSONL fixture: $file_path"
    echo "$errors" | tail -10
  } >&2
  exit 2
fi

exit 0

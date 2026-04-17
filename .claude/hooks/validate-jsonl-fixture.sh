#!/usr/bin/env bash
# PostToolUse hook: 在编辑/写入 tests/fixtures/*.jsonl 后逐行校验 JSON 合法性。
#
# JSONL fixture 被 cdt-parse / cdt-analyze / cdt-api 的集成测试大量使用，
# 手写时漏一个引号或闭括号会让测试在 parse 阶段炸，错误指向测试代码而非 fixture；
# 本 hook 在编辑瞬间暴露语法错误，省掉 cargo test → 追溯 → 定位的回合。
set -euo pipefail

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

if [[ -z "$file_path" || "$file_path" != *.jsonl ]]; then
  exit 0
fi
if [[ "$file_path" != *tests/fixtures/* ]]; then
  exit 0
fi
if [[ ! -f "$file_path" ]]; then
  exit 0
fi

if ! output=$(python3 - "$file_path" <<'PY' 2>&1
import json, sys
path = sys.argv[1]
errors = []
with open(path, "r", encoding="utf-8") as f:
    for i, line in enumerate(f, 1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            json.loads(stripped)
        except json.JSONDecodeError as e:
            errors.append(f"line {i}: {e.msg} (col {e.colno})")
if errors:
    for e in errors:
        print(e)
    sys.exit(1)
PY
); then
  {
    echo "invalid JSONL fixture: $file_path"
    echo "$output"
  } >&2
  exit 2
fi

exit 0

#!/usr/bin/env bash
# PreToolUse hook: 禁止直接编辑 openspec/specs/**。
# 项目约定所有 spec 变更走 openspec/changes/<name>/specs/ 的 delta，
# 再由 opsx:archive 时 sync 回主 spec。直接改主 spec 会绕过该流程。
#
# 例外：若环境变量 OPSX_ALLOW_DIRECT_SPEC_EDIT=1 则放行，给 opsx:archive
# 内部的 sync 步骤用。
set -euo pipefail

if [[ "${OPSX_ALLOW_DIRECT_SPEC_EDIT:-0}" == "1" ]]; then
  exit 0
fi

file_path=$(python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

if [[ -z "$file_path" ]]; then
  exit 0
fi

case "$file_path" in
  */openspec/specs/*)
    cat >&2 <<EOF
直接编辑 openspec/specs/ 会绕过 opsx delta 工作流。
请在 openspec/changes/<name>/specs/<capability>/spec.md 写 delta，
再由 /opsx:archive 时 sync 回主 spec。

如果确认要绕过（例如 sync 脚本内部调用），设 OPSX_ALLOW_DIRECT_SPEC_EDIT=1。
EOF
    exit 2
    ;;
esac

exit 0

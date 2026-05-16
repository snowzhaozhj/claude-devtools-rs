#!/usr/bin/env bash
# 校验三处 Tauri command 清单 1:1 同步：
#   1. crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS
#   2. ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS
#   3. src-tauri/src/lib.rs::invoke_handler!(tauri::generate_handler![...])
#
# 已知例外：`list_sessions_sync` 是 LocalDataApi 公开方法但**不**在 invoke_handler
# （仅供 HTTP server），故本脚本不应在任一三处列表中看到它——若出现即视为错配。
#
# 退出码：
#   0 三处一致
#   1 不一致（diff 打在 stderr）
#   2 提取失败（缺文件、清单块未找到等）
#
# 反 corner case：
#   - 多行 macro 块（generate_handler 跨行）—— awk 状态机扫描区间
#   - 行内 `// xxx`、`/* xxx */` 单行注释 —— 提取前剥离
#   - 整行注释（`// list_xxx,`）—— 跳过以注释起头的行
#   - 行尾逗号、尾随空格 —— 正则容错
set -euo pipefail

project_dir="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$project_dir"

CONTRACT="crates/cdt-api/tests/ipc_contract.rs"
MOCK="ui/src/lib/tauriMock.ts"
LIB="src-tauri/src/lib.rs"

for f in "$CONTRACT" "$MOCK" "$LIB"; do
  if [[ ! -f "$f" ]]; then
    echo "[check-ipc-command-sync] FATAL: 文件不存在 $f" >&2
    exit 2
  fi
done

extract_expected() {
  awk '
    /^pub const EXPECTED_TAURI_COMMANDS:[[:space:]]/ { in_block=1; next }
    in_block && /^\];/                              { exit }
    in_block {
      line = $0
      # 剥离行内 // 注释
      sub(/\/\/.*$/, "", line)
      sub(/\/\*.*\*\//, "", line)
      # 跳过空行
      gsub(/[[:space:]]+/, " ", line)
      sub(/^ /, "", line); sub(/ $/, "", line)
      if (line == "") next
      # 提取 "xxx"
      while (match(line, /"[a-zA-Z_][a-zA-Z0-9_]*"/) > 0) {
        tok = substr(line, RSTART+1, RLENGTH-2)
        print tok
        line = substr(line, RSTART+RLENGTH)
      }
    }
  ' "$CONTRACT" | sort -u
}

extract_known() {
  awk '
    /^const KNOWN_TAURI_COMMANDS:[[:space:]]/ { in_block=1; next }
    in_block && /^\][[:space:]]+as[[:space:]]+const/ { exit }
    in_block {
      line = $0
      sub(/\/\/.*$/, "", line)
      sub(/\/\*.*\*\//, "", line)
      gsub(/[[:space:]]+/, " ", line)
      sub(/^ /, "", line); sub(/ $/, "", line)
      if (line == "") next
      # 提取 '\''xxx'\''
      while (match(line, /'\''[a-zA-Z_][a-zA-Z0-9_]*'\''/) > 0) {
        tok = substr(line, RSTART+1, RLENGTH-2)
        print tok
        line = substr(line, RSTART+RLENGTH)
      }
    }
  ' "$MOCK" | sort -u
}

extract_handler() {
  awk '
    /tauri::generate_handler!\[/ { in_block=1; next }
    in_block && /^[[:space:]]*\]\)/ { exit }
    in_block {
      line = $0
      # 剥离行内 // 注释
      sub(/\/\/.*$/, "", line)
      sub(/\/\*.*\*\//, "", line)
      # 去前后空白
      sub(/^[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      # 去行尾逗号
      sub(/,$/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line == "") next
      # 整行注释（开头是 // 或 /*）已被前面 sub 清掉变空行
      # 一行可能有多个 ident（罕见但允许），用逗号 split
      n = split(line, parts, ",")
      for (i = 1; i <= n; i++) {
        tok = parts[i]
        sub(/^[[:space:]]+/, "", tok); sub(/[[:space:]]+$/, "", tok)
        if (tok ~ /^[a-z_][a-zA-Z0-9_]*$/) {
          print tok
        }
      }
    }
  ' "$LIB" | sort -u
}

expected_file=$(mktemp)
known_file=$(mktemp)
handler_file=$(mktemp)
trap 'rm -f "$expected_file" "$known_file" "$handler_file"' EXIT

extract_expected > "$expected_file"
extract_known    > "$known_file"
extract_handler  > "$handler_file"

# 任一为空 → 提取失败（清单块语法被改了导致 awk 未匹配）
for label in expected:$expected_file known:$known_file handler:$handler_file; do
  name=${label%%:*}
  path=${label#*:}
  if [[ ! -s "$path" ]]; then
    echo "[check-ipc-command-sync] FATAL: $name 列表提取为空——可能 awk 未匹配到 list 块。" >&2
    exit 2
  fi
done

# list_sessions_sync 一旦出现在三处任一即视为错配
forbidden="list_sessions_sync"
for label in expected:$expected_file known:$known_file handler:$handler_file; do
  name=${label%%:*}
  path=${label#*:}
  if grep -q "^${forbidden}$" "$path"; then
    echo "[check-ipc-command-sync] FATAL: '${forbidden}' MUST NOT 出现在 $name 列表中（它仅供 HTTP server，不是 Tauri command）。" >&2
    exit 1
  fi
done

fail=0
diff_pair() {
  local a_label=$1 a_file=$2 b_label=$3 b_file=$4
  local only_a only_b
  only_a=$(comm -23 "$a_file" "$b_file" || true)
  only_b=$(comm -13 "$a_file" "$b_file" || true)
  if [[ -n "$only_a" || -n "$only_b" ]]; then
    fail=1
    {
      echo "[check-ipc-command-sync] 不一致：${a_label} vs ${b_label}"
      if [[ -n "$only_a" ]]; then
        echo "  仅在 ${a_label}："
        printf '    - %s\n' $only_a
      fi
      if [[ -n "$only_b" ]]; then
        echo "  仅在 ${b_label}："
        printf '    - %s\n' $only_b
      fi
    } >&2
  fi
}

diff_pair "EXPECTED_TAURI_COMMANDS"  "$expected_file" "invoke_handler!" "$handler_file"
diff_pair "KNOWN_TAURI_COMMANDS"     "$known_file"    "invoke_handler!" "$handler_file"
diff_pair "EXPECTED_TAURI_COMMANDS"  "$expected_file" "KNOWN_TAURI_COMMANDS" "$known_file"

if (( fail )); then
  {
    echo
    echo "三处来源："
    echo "  1. EXPECTED_TAURI_COMMANDS  → $CONTRACT"
    echo "  2. KNOWN_TAURI_COMMANDS     → $MOCK"
    echo "  3. invoke_handler!          → $LIB"
    echo "改动 Tauri command 时 MUST 三处同步更新。"
  } >&2
  exit 1
fi

echo "[check-ipc-command-sync] OK ($(wc -l < "$expected_file" | tr -d ' ') commands)"
exit 0

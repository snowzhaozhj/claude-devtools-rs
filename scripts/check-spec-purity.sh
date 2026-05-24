#!/usr/bin/env bash
# 扫 openspec/specs/<cap>/spec.md（主 spec）与 openspec/changes/<slug>/specs/<cap>/spec.md
# （active change delta）的六类反模式（与 openspec/config.yaml::rules.specs 对齐）：
#   1. 内部模块/类/函数名（`crate::module::fn` 等）
#   2. 源文件路径（.rs / .ts / .svelte / crates/ / src-tauri/src/ / ui/src/）
#   3. commit hash / PR# / issue#（属诊断溯源，应进 design.md）
#   4. 实测诊断数据（KB / MB / ms / 「实测」/ baseline）
#   5. 回滚开关 const 名（`OMIT_*` / `CROSS_*` / `STALE_*` 等）
#   6. 库与框架选型（tokio / serde / axum / tauri-plugin-X / vitest / playwright 等）
#
# 跳过 openspec/changes/archive/**（历史快照冻结）。
#
# 模式：
#   --baseline    打印每 spec 的当前违规计数到 stdout（用于刷新 baseline 文件）
#   --report      详细报告每 spec 各类反模式分布 + 命中行示例
#   （默认）       与 scripts/spec-purity-baseline.txt 对比；任何 spec 超出
#                 baseline → exit 1（ratchet 模式，只允许下降）
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT"
BASELINE_FILE="scripts/spec-purity-baseline.txt"

scan_one() {
  local file="$1"
  local p1 p2 p3 p4 p5 p6 total
  p1=$(grep -cE '`[a-z][a-z_]+::[A-Za-z_:]+' "$file" || true)
  p2=$(grep -cE '\.(rs|ts|svelte|tsx|jsx)\b|/src/|crates/|src-tauri/|cdt-[a-z]+/src' "$file" || true)
  p3=$(grep -cE '\b(commit|PR|issue)\s*#?[0-9a-f]{4,}|#[0-9]{2,}\b' "$file" || true)
  p4=$(grep -cE '[0-9]+\s*(KB|MB|byte|ms|µs)\b|实测|baseline' "$file" || true)
  p5=$(grep -cE '`(OMIT_|CROSS_|STALE_)[A-Z_]+`|`[A-Z][A-Z_]{4,}`' "$file" || true)
  p6=$(grep -cE '\b(tokio|serde|axum|broadcast::|tracing::|invoke_handler!)\b|#\[serde' "$file" || true)
  total=$((p1 + p2 + p3 + p4 + p5 + p6))
  echo "$total $p1 $p2 $p3 $p4 $p5 $p6"
}

# 收集所有 spec key → file 路径（排除 archive）
declare -a KEYS FILES
while IFS= read -r f; do
  cap=$(basename "$(dirname "$f")")
  KEYS+=("spec/$cap")
  FILES+=("$f")
done < <(find openspec/specs -mindepth 2 -maxdepth 2 -name 'spec.md' -type f 2>/dev/null | sort)

while IFS= read -r f; do
  # f 形如 openspec/changes/<slug>/specs/<cap>/spec.md
  slug=$(echo "$f" | awk -F/ '{print $3}')
  cap=$(basename "$(dirname "$f")")
  KEYS+=("change/$slug/$cap")
  FILES+=("$f")
done < <(find openspec/changes -mindepth 5 -maxdepth 5 -name 'spec.md' -type f -not -path 'openspec/changes/archive/*' 2>/dev/null | sort)

if [[ "${1:-}" == "--report" ]]; then
  printf "%-50s %-6s %-3s %-3s %-3s %-3s %-3s %-3s\n" "spec key" "total" "p1" "p2" "p3" "p4" "p5" "p6"
  printf "%-50s %-6s %-3s %-3s %-3s %-3s %-3s %-3s\n" "(p1=mod-path p2=src-path p3=commit p4=metric p5=const p6=lib)" "" "" "" "" "" "" ""
  for i in "${!KEYS[@]}"; do
    read -r total p1 p2 p3 p4 p5 p6 <<< "$(scan_one "${FILES[$i]}")"
    [[ "$total" -gt 0 ]] && printf "%-50s %-6d %-3d %-3d %-3d %-3d %-3d %-3d\n" "${KEYS[$i]}" "$total" "$p1" "$p2" "$p3" "$p4" "$p5" "$p6"
  done | sort -k2 -rn
  exit 0
fi

if [[ "${1:-}" == "--baseline" ]]; then
  for i in "${!KEYS[@]}"; do
    read -r total _ <<< "$(scan_one "${FILES[$i]}")"
    echo "${KEYS[$i]} $total"
  done | sort
  exit 0
fi

# 默认：与 baseline 对比
if [[ ! -f "$BASELINE_FILE" ]]; then
  echo "✗ baseline 文件不存在：$BASELINE_FILE" >&2
  echo "  跑：bash scripts/check-spec-purity.sh --baseline > $BASELINE_FILE" >&2
  exit 2
fi

tmp_current=$(mktemp)
trap 'rm -f "$tmp_current"' EXIT
for i in "${!KEYS[@]}"; do
  read -r total _ <<< "$(scan_one "${FILES[$i]}")"
  echo "${KEYS[$i]} $total" >> "$tmp_current"
done

awk -v baseline_file="$BASELINE_FILE" '
  BEGIN {
    while ((getline line < baseline_file) > 0) {
      if (line == "" || line ~ /^#/) continue
      split(line, a, " ")
      if (a[1] != "") base[a[1]] = a[2] + 0
    }
    close(baseline_file)
    failed = 0
    n_exc = 0; n_new = 0; n_imp = 0
  }
  {
    key = $1; total = $2 + 0; seen[key] = 1
    if (!(key in base)) {
      if (total > 0) { new_arr[++n_new] = key ":" total; failed = 1 }
    } else if (total > base[key]) {
      exc_arr[++n_exc] = key ": baseline=" base[key] " now=" total " (+" (total - base[key]) ")"
      failed = 1
    } else if (total < base[key]) {
      imp_arr[++n_imp] = key ": baseline=" base[key] " now=" total " (-" (base[key] - total) ")"
    }
  }
  END {
    n_rem = 0
    for (k in base) if (!(k in seen)) rem_arr[++n_rem] = k

    if (failed) {
      print "" > "/dev/stderr"
      print "✗ spec 反模式 ratchet 拦截 — 任一 spec 超过 baseline 已记录违规计数即拒" > "/dev/stderr"
      print "" > "/dev/stderr"
      if (n_exc > 0) {
        print "超出 baseline:" > "/dev/stderr"
        for (i = 1; i <= n_exc; i++) print "  - " exc_arr[i] > "/dev/stderr"
      }
      if (n_new > 0) {
        print "新增 spec（baseline 无记录）含反模式:" > "/dev/stderr"
        for (i = 1; i <= n_new; i++) print "  - " new_arr[i] > "/dev/stderr"
      }
      print "" > "/dev/stderr"
      print "修法:" > "/dev/stderr"
      print "  1) bash scripts/check-spec-purity.sh --report  # 看具体哪类反模式 + 命中行" > "/dev/stderr"
      print "  2) 把函数名 / 源文件路径 / commit 引用 / 实测数据 / 回滚 const / 库选型迁到 design.md" > "/dev/stderr"
      print "  3) 若清理后 total 下降: bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt" > "/dev/stderr"
      print "" > "/dev/stderr"
      print "spec 反模式定义见 openspec/config.yaml::rules.specs" > "/dev/stderr"
      exit 1
    }

    if (n_imp > 0) {
      print "✓ spec 污染下降（清理后刷新 baseline）:"
      for (i = 1; i <= n_imp; i++) print "  - " imp_arr[i]
      print ""
      print "刷新基线: bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt"
    }
    if (n_rem > 0) {
      print "ℹ baseline 中以下 key 在当前 specs 不存在（spec 被删 / 改名）:"
      for (i = 1; i <= n_rem; i++) print "  - " rem_arr[i]
      print "  考虑刷新 baseline 移除这些项。"
    }
    print "✓ spec purity check 通过（无 spec 超过 baseline）"
  }
' "$tmp_current"

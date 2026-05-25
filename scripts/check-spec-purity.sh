#!/usr/bin/env bash
# 扫 openspec/specs/<cap>/spec.md（主 spec）与 openspec/changes/<slug>/specs/<cap>/spec.md
# （active change delta）的六类反模式（与 openspec/config.yaml::rules.specs 对齐）：
#   1. 内部模块/类/函数名（`crate::module::fn` 等）
#   2. 源文件路径（.rs / .ts / .svelte / crates/ / src-tauri/src/ / ui/src/）
#   3. commit hash / PR# / issue#（裸 12+ 位 hex 或 backtick 内 7+ 位 hex）
#   4. 实测诊断数据（KB / MB / ms / 「实测」/ baseline）
#   5. 回滚/实现开关 const（`OMIT_*` / `CROSS_*` / `STALE_*` / `LEGACY_*` /
#      `ENABLE_*` / `DISABLE_*` / `USE_*` / `FORCE_*` / `*_THRESHOLD` /
#      `*_FLAG` / `*_ENABLED` / `*_DISABLED`）；不抓 RFC2119 关键词与协议常量
#   6. 库与框架选型（tokio / serde / axum / vitest / playwright / svelte-check /
#      tauri-plugin-X / @tauri-apps/X / broadcast:: / tracing:: / tauri:: /
#      invoke_handler! / #[serde 等具名引用）
#
# 跳过 openspec/changes/archive/**（历史快照冻结）。
#
# 模式：
#   --baseline    打印每 spec 的当前违规计数到 stdout（用于刷新 baseline 文件）
#   --report      详细报告每 spec 各类反模式分布 + 命中行示例
#   （默认）       与 scripts/spec-purity-baseline.txt 对比；**单向 ratchet**：
#                 超 baseline 拒（防恶化）；低于 baseline 仅信息提示，不拒。
#                 env SPEC_PURITY_STRICT=1 切回**双向 ratchet**（低于 baseline
#                 也拒，强制开发者同 PR 刷新——防 silent degradation）。
#
# 历史：原默认是双向 ratchet。spec-overhaul-file-watching-pilot 试点期发现
# 双向的"强制同 PR 刷新"对每个 spec 改动都强加 baseline 同步思维负担，且
# silent degradation 在实践中罕见——switch 到单向 ratchet 减负，仍保住核心
# 防恶化能力。需要双向 strict gate 时通过 env 显式 opt-in。
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT"
BASELINE_FILE="scripts/spec-purity-baseline.txt"

re_p1='`[a-z][a-z_]+::[A-Za-z_:]+'
re_p2='\.(rs|ts|svelte|tsx|jsx)\b|/src/|crates/|src-tauri/|cdt-[a-z]+/src'
# p3: 显式 commit/PR/issue 前缀；裸 #NNN；backtick 内 7-40 位 hex；裸文本 12-40 位 hex
re_p3='\b(commit|PR|issue)\s*#?[0-9a-f]{4,}|#[0-9]{2,}\b|`[0-9a-f]{7,40}`|\b[0-9a-f]{12,40}\b'
re_p4='[0-9]+\s*(KB|MB|byte|ms|µs)\b|实测|baseline'
# p5: 仅抓回滚/实现开关前缀清单 + 阈值/标志后缀，不抓 RFC2119 关键词或协议常量
re_p5='`(OMIT|CROSS|STALE|LEGACY|ENABLE|DISABLE|USE|FORCE|SKIP|PRESERVE)_[A-Z0-9_]+`|`[A-Z][A-Za-z0-9_]*_(THRESHOLD|FLAG|ENABLED|DISABLED|TIMEOUT|LIMIT)`'
# p6: 精准包/工具名（不抓裸 Tauri 这种合法协议名）
re_p6='\b(tokio|serde|axum|vitest|playwright|svelte-check|reqwest|hyper)\b|\btauri-plugin-[a-z0-9-]+\b|@tauri-apps/[a-z0-9_/-]+|\b(broadcast|tracing|tauri)::|invoke_handler!|#\[serde'

scan_one() {
  local file="$1"
  local p1 p2 p3 p4 p5 p6 total
  p1=$(grep -cE "$re_p1" "$file" || true)
  p2=$(grep -cE "$re_p2" "$file" || true)
  p3=$(grep -cE "$re_p3" "$file" || true)
  p4=$(grep -cE "$re_p4" "$file" || true)
  p5=$(grep -cE "$re_p5" "$file" || true)
  p6=$(grep -cE "$re_p6" "$file" || true)
  total=$((p1 + p2 + p3 + p4 + p5 + p6))
  echo "$total $p1 $p2 $p3 $p4 $p5 $p6"
}

# --report 时按文件打印每类前 3 条 grep -n 命中行
print_examples_for() {
  local file="$1"
  local key="$2"
  local emitted=0
  for entry in "p1=$re_p1" "p2=$re_p2" "p3=$re_p3" "p4=$re_p4" "p5=$re_p5" "p6=$re_p6"; do
    local label="${entry%%=*}"
    local re="${entry#*=}"
    local hits
    hits=$(grep -nE "$re" "$file" 2>/dev/null | head -3 || true)
    if [[ -n "$hits" ]]; then
      if [[ "$emitted" -eq 0 ]]; then
        echo ""
        echo "## $key  ($file)"
        emitted=1
      fi
      echo "  [$label]"
      echo "$hits" | sed 's/^/    /'
    fi
  done
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
  # 相对 `openspec/changes` 起点深度 = 4（slug/specs/cap/spec.md）；
  # archive 路径深度 = 5（archive/<date-slug>/specs/cap/spec.md），靠 -not -path 兜底排除
  slug=$(echo "$f" | awk -F/ '{print $3}')
  cap=$(basename "$(dirname "$f")")
  KEYS+=("change/$slug/$cap")
  FILES+=("$f")
done < <(find openspec/changes -mindepth 4 -maxdepth 4 -name 'spec.md' -type f -not -path 'openspec/changes/archive/*' 2>/dev/null | sort)

if [[ "${1:-}" == "--report" ]]; then
  echo "p1=mod-path  p2=src-path  p3=commit/hash  p4=metric  p5=impl-flag  p6=lib/framework"
  echo ""
  printf "%-50s %-6s %-3s %-3s %-3s %-3s %-3s %-3s\n" "spec key" "total" "p1" "p2" "p3" "p4" "p5" "p6"
  for i in "${!KEYS[@]}"; do
    read -r total p1 p2 p3 p4 p5 p6 <<< "$(scan_one "${FILES[$i]}")"
    [[ "$total" -gt 0 ]] && printf "%-50s %-6d %-3d %-3d %-3d %-3d %-3d %-3d\n" "${KEYS[$i]}" "$total" "$p1" "$p2" "$p3" "$p4" "$p5" "$p6"
  done | sort -k2 -rn
  echo ""
  echo "=== 命中行示例（每 spec 每类前 3 条） ==="
  for i in "${!KEYS[@]}"; do
    read -r total _ <<< "$(scan_one "${FILES[$i]}")"
    [[ "$total" -gt 0 ]] && print_examples_for "${FILES[$i]}" "${KEYS[$i]}"
  done
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

# 单向 ratchet（默认）：超 baseline 拒，低于 baseline 仅信息提示。
# 双向 ratchet（opt-in）：env SPEC_PURITY_STRICT=1 时强制下降同 commit 刷 baseline。
# 兼容老 env：SPEC_PURITY_ALLOW_DECREASE=0 等价 STRICT=1（向后兼容已有 CI 配置）。
strict_mode="${SPEC_PURITY_STRICT:-0}"
if [[ "${SPEC_PURITY_ALLOW_DECREASE:-1}" == "0" ]]; then
  strict_mode="1"
fi

awk -v baseline_file="$BASELINE_FILE" -v strict_mode="$strict_mode" '
  BEGIN {
    while ((getline line < baseline_file) > 0) {
      if (line == "" || line ~ /^#/) continue
      split(line, a, " ")
      if (a[1] != "") base[a[1]] = a[2] + 0
    }
    close(baseline_file)
    failed = 0
    n_exc = 0; n_new = 0; n_imp = 0; n_rem = 0
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
    for (k in base) if (!(k in seen)) rem_arr[++n_rem] = k

    decrease_fail = (n_imp > 0 && strict_mode == "1") ? 1 : 0
    removed_fail = (n_rem > 0 && strict_mode == "1") ? 1 : 0

    if (failed) {
      print "" > "/dev/stderr"
      print "✗ spec 反模式 ratchet 拦截 — 超过 baseline" > "/dev/stderr"
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
      print "  1) bash scripts/check-spec-purity.sh --report  # 看哪类反模式 + 命中行" > "/dev/stderr"
      print "  2) 把函数名 / 源文件路径 / commit 引用 / 实测数据 / 回滚 const / 库选型迁到 design.md" > "/dev/stderr"
      print "  3) 清理后让 total 下降，再跑 --baseline 刷新基线（与改动同 commit 落地）" > "/dev/stderr"
      print "" > "/dev/stderr"
      print "spec 反模式定义见 openspec/config.yaml::rules.specs" > "/dev/stderr"
      exit 1
    }

    if (decrease_fail || removed_fail) {
      print "" > "/dev/stderr"
      print "✗ STRICT 模式：spec 污染下降但 baseline 未同步刷新（强制防 silent degradation）" > "/dev/stderr"
      print "" > "/dev/stderr"
      if (n_imp > 0) {
        print "spec 污染下降:" > "/dev/stderr"
        for (i = 1; i <= n_imp; i++) print "  - " imp_arr[i] > "/dev/stderr"
      }
      if (n_rem > 0) {
        print "baseline 残留（spec 被删 / 改名）:" > "/dev/stderr"
        for (i = 1; i <= n_rem; i++) print "  - " rem_arr[i] > "/dev/stderr"
      }
      print "" > "/dev/stderr"
      print "修法（与本次改动同 commit）:" > "/dev/stderr"
      print "  bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt" > "/dev/stderr"
      print "" > "/dev/stderr"
      print "或退出 STRICT 模式：unset SPEC_PURITY_STRICT 与 SPEC_PURITY_ALLOW_DECREASE" > "/dev/stderr"
      exit 1
    }

    if (n_imp > 0) {
      print "ℹ spec 污染下降（baseline 比当前态宽松，未拒——可手动 --baseline 刷新收紧 ratchet）:"
      for (i = 1; i <= n_imp; i++) print "  - " imp_arr[i]
    }
    if (n_rem > 0) {
      print "ℹ baseline 残留 key（spec 已删 / 改名；可手动 --baseline 清理）:"
      for (i = 1; i <= n_rem; i++) print "  - " rem_arr[i]
    }
    print "✓ spec purity check 通过"
  }
' "$tmp_current"

#!/usr/bin/env bash
# spec 反模式检查（精简版）
#
# 只拦真正有害的实现细节泄漏，不拦合法行为契约内容。
#
# 3 类规则：
#   p2: 精确源文件路径（crates/*/src/*.rs、src-tauri/src/*.rs、ui/src/*.ts 等）
#       放行 PascalCase.svelte（UI surface 名）和裸 crate 名
#   p3: 过期锚点（40 字符 hex commit hash、backtick 短 hash、L+数字行号、裸 PR#/issue#）
#   p4: 实测数据冒充 SLA（"实测/bench 结果/measured"但排除同行有 SHALL/MUST/Scenario 的）
#
# 行尾 <!-- spec-purity: ok --> 跳过该行（显式标记合法使用）。
#
# 模式：
#   --report    详细报告命中行
#   --test      跑内置自测样本验证规则正确性
#   （默认）    硬阈值 gate：单 spec > 5 fail
#
# 已删除：baseline ratchet（维护税高 + false positive 多）
set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT"

THRESHOLD="${SPEC_PURITY_THRESHOLD:-5}"

# p2: 精确到源文件的内部路径
# 匹配 crates/<name>/src/ 或 src-tauri/src/ 或 ui/src/ 后跟具体文件
re_p2='crates/[a-z_-]+/src/[^ ]+\.rs|src-tauri/src/[^ ]+\.rs|ui/src/[^ ]+\.(ts|svelte)'

# p3: 过期锚点
# - 40 字符 hex（完整 commit hash）
# - backtick 内 7-40 位 hex（短 hash）
# - L+数字行号引用（如 L42、L100-L150）
# - 显式 PR#/issue# 引用
re_p3='\b[0-9a-f]{40}\b|`[0-9a-f]{7,40}`|\bL[0-9]+(-L?[0-9]+)?\b|(PR|issue)\s*#[0-9]+'

scan_one() {
  local file="$1"
  local suppress_re='<!-- spec-purity: ok -->'
  local p2 p3 p4 total

  p2=$(grep -vE "$suppress_re" "$file" | grep -cE "$re_p2" || true)
  p3=$(grep -vE "$suppress_re" "$file" | grep -cE "$re_p3" || true)
  # p4: 匹配实测数据（排除含 SHALL/MUST/SLA/预算/Scenario 的行——那是契约）
  # "baseline" 太泛（domain concept + scenario 名），只保留"实测"+数字单位组合
  p4=$(grep -vE "$suppress_re" "$file" | grep -iE '实测|bench\s*(结果|result|数据)|measured' | grep -civE 'SHALL|MUST|SLA|预算|budget|Scenario' || true)

  total=$((p2 + p3 + p4))
  echo "$total $p2 $p3 $p4"
}

print_examples_for() {
  local file="$1"
  local key="$2"
  local suppress_re='<!-- spec-purity: ok -->'
  local emitted=0

  for entry in "p2-src-path=$re_p2" "p3-stale-anchor=$re_p3"; do
    local label="${entry%%=*}"
    local re="${entry#*=}"
    local hits
    hits=$(grep -nE "$re" "$file" 2>/dev/null | grep -vF "$suppress_re" | head -3 || true)
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

  # p4 单独处理（双重过滤）
  local p4_hits
  p4_hits=$(grep -nE '实测|bench\s*(结果|result|数据)|measured' "$file" 2>/dev/null | grep -vF "$suppress_re" | grep -ivE 'SHALL|MUST|SLA|预算|budget|Scenario' | head -3 || true)
  if [[ -n "$p4_hits" ]]; then
    if [[ "$emitted" -eq 0 ]]; then
      echo ""
      echo "## $key  ($file)"
    fi
    echo "  [p4-observed-data]"
    echo "$p4_hits" | sed 's/^/    /'
  fi
}

# --test: 内置自测
if [[ "${1:-}" == "--test" ]]; then
  SAMPLES_DIR="tests/spec-purity-samples"
  pass=0; fail=0

  if [[ -f "$SAMPLES_DIR/should-pass.md" ]]; then
    read -r total _ <<< "$(scan_one "$SAMPLES_DIR/should-pass.md")"
    if [[ "$total" -eq 0 ]]; then
      echo "✓ should-pass.md: 0 violations"; pass=$((pass + 1))
    else
      echo "✗ should-pass.md: expected 0, got $total"; fail=$((fail + 1))
    fi
  fi

  if [[ -f "$SAMPLES_DIR/should-fail.md" ]]; then
    read -r total _ <<< "$(scan_one "$SAMPLES_DIR/should-fail.md")"
    if [[ "$total" -gt 0 ]]; then
      echo "✓ should-fail.md: $total violations (expected >0)"; pass=$((pass + 1))
    else
      echo "✗ should-fail.md: expected >0, got 0"; fail=$((fail + 1))
    fi
  fi

  echo ""
  echo "Results: $pass passed, $fail failed"
  [[ "$fail" -eq 0 ]] && exit 0 || exit 1
fi

# 收集所有 spec（排除 archive）
declare -a KEYS FILES
while IFS= read -r f; do
  cap=$(basename "$(dirname "$f")")
  KEYS+=("spec/$cap")
  FILES+=("$f")
done < <(find openspec/specs -mindepth 2 -maxdepth 2 -name 'spec.md' -type f 2>/dev/null | sort)

while IFS= read -r f; do
  slug=$(echo "$f" | awk -F/ '{print $3}')
  cap=$(basename "$(dirname "$f")")
  KEYS+=("change/$slug/$cap")
  FILES+=("$f")
done < <(find openspec/changes -mindepth 4 -maxdepth 4 -name 'spec.md' -type f -not -path 'openspec/changes/archive/*' 2>/dev/null | sort)

if [[ ${#KEYS[@]} -eq 0 ]]; then
  echo "✓ spec purity: 无 spec 文件"
  exit 0
fi

if [[ "${1:-}" == "--report" ]]; then
  echo "规则：p2=源文件路径  p3=过期锚点  p4=实测数据（排除 SLA）"
  echo "阈值：单 spec > ${THRESHOLD} 即 fail"
  echo ""
  printf "%-50s %-6s %-3s %-3s %-3s\n" "spec key" "total" "p2" "p3" "p4"
  for i in "${!KEYS[@]}"; do
    read -r total p2 p3 p4 <<< "$(scan_one "${FILES[$i]}")"
    [[ "$total" -gt 0 ]] && printf "%-50s %-6d %-3d %-3d %-3d\n" "${KEYS[$i]}" "$total" "$p2" "$p3" "$p4"
  done | sort -k2 -rn || true
  echo ""
  echo "=== 命中行示例 ==="
  for i in "${!KEYS[@]}"; do
    read -r total _ <<< "$(scan_one "${FILES[$i]}")"
    [[ "$total" -gt 0 ]] && print_examples_for "${FILES[$i]}" "${KEYS[$i]}"
  done
  exit 0
fi

# 默认模式：硬阈值 gate
failed=0
fail_list=""
warn_list=""

for i in "${!KEYS[@]}"; do
  read -r total p2 p3 p4 <<< "$(scan_one "${FILES[$i]}")"
  if [[ "$total" -gt "$THRESHOLD" ]]; then
    fail_list+="  ✗ ${KEYS[$i]}: $total (p2=$p2 p3=$p3 p4=$p4)\n"
    failed=1
  elif [[ "$total" -gt 0 ]]; then
    warn_list+="  · ${KEYS[$i]}: $total (p2=$p2 p3=$p3 p4=$p4)\n"
  fi
done

if [[ "$failed" -eq 1 ]]; then
  echo "" >&2
  echo "✗ spec purity: 以下 spec 超阈值 (>${THRESHOLD})" >&2
  echo -e "$fail_list" >&2
  echo "修法：" >&2
  echo "  1) bash scripts/check-spec-purity.sh --report  # 看命中行" >&2
  echo "  2) 迁移实测数据到 design.md；删过期 hash/行号；精确源路径改为 capability 引用" >&2
  echo "  3) 合法使用加 <!-- spec-purity: ok --> 行尾注释" >&2
  exit 1
fi

if [[ -n "$warn_list" ]]; then
  echo "ℹ spec purity 通过（以下 spec 有少量命中，未超阈值）:"
  echo -e "$warn_list"
fi
echo "✓ spec purity check 通过 (threshold=${THRESHOLD})"

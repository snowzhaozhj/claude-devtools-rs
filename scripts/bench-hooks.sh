#!/usr/bin/env bash
# 跑所有 .claude/hooks/*.sh 单次模拟耗时，对比 .claude/rules/perf.md "Hook 性能" 段 预算。
#
# 用法：
#   bash scripts/bench-hooks.sh              # 默认：99% 路径（不命中关键模式）
#   bash scripts/bench-hooks.sh --hot        # 1% 路径（命中关键模式，跑真业务）
#
# 输出：表格列出每个 hook 的 wall time + 是否超预算
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.claude/hooks"

mode="${1:-cold}"
case "$mode" in
  --hot|hot) mode="hot" ;;
  *) mode="cold" ;;
esac

# fixture：模拟 PreToolUse Bash hook 的 input（cold = 不命中关键模式 / hot = 命中）
if [[ "$mode" == "cold" ]]; then
  INPUT_BASH='{"tool_name":"Bash","tool_input":{"command":"echo hello world"}}'
  INPUT_EDIT='{"tool_name":"Edit","tool_input":{"file_path":"/tmp/foo.txt","old_string":"a","new_string":"b"}}'
  # 物理下限 ~56ms（bash 启动 28ms + stdin 读 25ms + 必要 init）；预算留 4ms 余量
  budget_ms=60
  echo "=== Cold path（99% 调用，不命中关键模式，应快速 exit 0）==="
else
  INPUT_BASH='{"tool_name":"Bash","tool_input":{"command":"git commit -m test"}}'
  INPUT_EDIT='{"tool_name":"Edit","tool_input":{"file_path":"'"$REPO_ROOT"'/src/foo.rs","old_string":"a","new_string":"b"}}'
  # Hot path 含 jq 提取（+25ms）+ 真业务（git / openspec / cargo 启动）
  budget_ms=300
  echo "=== Hot path（1% 调用，命中关键模式，跑真业务）==="
fi

printf "%-50s %12s %10s %s\n" "Hook" "Wall (ms)" "Budget" "Status"
printf '%.0s-' {1..100}; echo ""

total_ms=0
for hook in "$HOOKS_DIR"/*.sh; do
  name=$(basename "$hook")

  # 选 fixture：根据 hook 名推断它处理的 tool_name
  case "$name" in
    deny-edit-on-main.sh) input="$INPUT_EDIT" ;;
    *-after-rs-edit.sh|*-after-edit.sh|validate-jsonl-fixture.sh|invalidate-src-tauri-cache.sh)
      input="$INPUT_EDIT" ;;
    *) input="$INPUT_BASH" ;;
  esac

  # 跑 3 次取最小（最稳）
  best=999999
  for i in 1 2 3; do
    start=$(python3 -c 'import time; print(int(time.time()*1000))')
    printf '%s' "$input" | bash "$hook" >/dev/null 2>&1 || true
    end=$(python3 -c 'import time; print(int(time.time()*1000))')
    elapsed=$((end - start))
    if (( elapsed < best )); then best=$elapsed; fi
  done

  total_ms=$((total_ms + best))

  if (( best <= budget_ms )); then
    status="✓ OK"
  else
    status="✗ OVER"
  fi
  printf "%-50s %12d %10d %s\n" "$name" "$best" "$budget_ms" "$status"
done

printf '%.0s-' {1..100}; echo ""
printf "%-50s %12d\n" "TOTAL (累计开销 / Bash 或 Edit 工具调用)" "$total_ms"
echo ""
echo '预算依据：.claude/rules/perf.md "Hook 性能" 段'

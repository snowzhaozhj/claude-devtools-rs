#!/usr/bin/env bash
# PreToolUse Bash hook: 在 `git push` 之前检查是否有"已完成但未 archive"的 openspec change，
# 若有则阻断 push 让 Claude 先跑 openspec archive。
#
# 触发条件（全部满足才拦）：
# 1. settings.json matcher 已限制 tool_name == Bash（hook 内不再判）
# 2. tool_input.command 含 "git push"（单词边界精确匹配）
# 3. openspec list --json 至少一个 active change 满足 status=complete 或 completedTasks==totalTasks
#
# 性能预算（见 .claude/rules/hooks-performance.md）：
# - 99% 命中：case 预判直接 exit 0，~5ms
# - 1% 命中：jq 提取 + openspec list
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：不含 "git push" 直接放行
case "$input" in
  *'"command"'*'git push'*) ;;
  *) exit 0 ;;
esac

# 严谨提取（jq 失败 fallback sed）
command=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

# 单词边界精确匹配 git push（避免误匹配 git push-config 等）
if ! [[ "$command" =~ (^|[[:space:]&\;|])git[[:space:]]+push([[:space:]&\;|]|$) ]]; then
  exit 0
fi

if ! command -v openspec >/dev/null 2>&1; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# jq 解析 openspec list 输出（替代 python3，省 ~30ms）
completed=$(openspec list --json 2>/dev/null \
  | jq -r '.changes[] | select(.status == "complete" or (.totalTasks != null and .completedTasks == .totalTasks)) | .name' \
  2>/dev/null || true)

if [[ -z "$completed" ]]; then
  exit 0
fi

{
  echo "[release-tail-check] 阻塞 git push：以下 openspec change 已完成但未 archive。"
  echo "若 push 上去，CI 的 scripts/check-openspec-archives.sh 会失败。"
  echo ""
  echo "未 archive 的 change："
  printf '  - %s\n' $completed
  echo ""
  echo "下一步（archive 是原子操作：同时 mv 目录到 archive/ + sync delta 回主 spec）："
  for c in $completed; do
    echo "  openspec archive $c -y"
  done
  echo "  git add -A && git commit -m 'chore(opsx): archive ...' && git push"
  echo ""
  echo "完整流水线见 .claude/rules/opsx-apply-cadence.md \"发布尾段\"段。"
} >&2

exit 2

#!/usr/bin/env bash
# PreToolUse hook on Bash: 在 `git push` 之前检查是否有"已完成但未 archive"的
# openspec change。若有，阻断 push 让 Claude 先跑 `openspec archive`。
#
# 动机：opsx:apply 完成所有 task 后若直接 push，会触发 CI 的
# `scripts/check-openspec-archives.sh` 拦截。本 hook 在 push **前**就拦下，
# 避免 "push → CI 失败 → 修 → 重新 push" 的往返。
#
# 触发条件（全部满足才拦）：
# 1. tool_name == "Bash"
# 2. tool_input.command 含 `git push`
# 3. `openspec list --json` 里至少一个 active change 满足
#    `status == "complete"` 或 `completedTasks == totalTasks`
#
# 选 PreToolUse Bash 而不是 Stop：
# - 仅在 `git push` 关键节点触发，开销 < 每个 turn 跑一次的 Stop hook
# - 在 push 前阻断 = 真正避免 CI 拦截往返；Stop 是事后提醒
# - matcher 精准，不打扰普通对话或其他工具调用
set -euo pipefail

input=$(cat)

tool_name=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
if [[ "$tool_name" != "Bash" ]]; then
  exit 0
fi

command=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || true)
# 匹配 `git push`（单词边界，避免误匹配 `git push-config` 之类不存在的子命令）
if ! [[ "$command" =~ (^|[[:space:]&;|])git[[:space:]]+push([[:space:]&;|]|$) ]]; then
  exit 0
fi

if ! command -v openspec >/dev/null 2>&1; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

json=$(openspec list --json 2>/dev/null || echo '{"changes":[]}')

completed=$(printf '%s' "$json" | python3 -c '
import json, sys
data = json.load(sys.stdin)
out = []
for change in data.get("changes", []):
    name = change.get("name")
    status = change.get("status")
    completed_tasks = change.get("completedTasks")
    total_tasks = change.get("totalTasks")
    if name and (status == "complete" or (total_tasks is not None and completed_tasks == total_tasks)):
        out.append(name)
print("\n".join(out))
' 2>/dev/null || true)

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

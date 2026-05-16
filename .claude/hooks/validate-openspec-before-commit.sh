#!/usr/bin/env bash
# PreToolUse Bash hook: 检测到 `git commit` 命令时，对 staged 的 openspec/changes/*/specs/**
# 跑 `openspec validate <name> --strict`，任一失败阻断提交。
#
# 触发条件（全部满足才跑校验）：
# 1. settings.json matcher 已限制 tool_name == Bash（hook 内不再判，省 1 次 jq）
# 2. tool_input.command 以 "git commit" 起头
# 3. staged 至少有一个 openspec/changes/<name>/specs/**/*.md
#
# 性能预算（见 .claude/rules/hooks-performance.md）：
# - 99% 命中：case 预判直接 exit 0，~5ms
# - 1% 命中：jq 提取 + openspec validate
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：input 不含 "git commit" 直接放行（bash case 内置 0 fork）
case "$input" in
  *'"command"'*'git commit'*) ;;
  *) exit 0 ;;
esac

# 严谨提取（jq 比 python3 快 ~2.5×；fallback sed 兜底 jq 不可用）
command=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ "$command" != git\ commit* ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

staged=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null || true)
if [[ -z "$staged" ]]; then
  exit 0
fi

changes=$(printf '%s\n' "$staged" \
  | grep -E '^openspec/changes/[^/]+/specs/.+\.md$' \
  | cut -d/ -f3 \
  | sort -u || true)

if [[ -z "$changes" ]]; then
  exit 0
fi

failed=()
for change in $changes; do
  if ! out=$(openspec validate "$change" --strict 2>&1); then
    failed+=("$change")
    {
      echo "openspec validate --strict failed for change '$change':"
      echo "$out" | tail -20
      echo
    } >&2
  fi
done

if (( ${#failed[@]} > 0 )); then
  echo "Blocking git commit: fix delta spec errors above, or stage without the bad specs." >&2
  exit 2
fi

exit 0

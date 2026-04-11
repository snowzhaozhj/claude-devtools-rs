#!/usr/bin/env bash
# PreToolUse hook: 当检测到 `git commit` 命令即将执行时，扫描 staged 文件里
# 属于 openspec/changes/*/specs/** 的条目，对它们所属的 change 跑
# `openspec validate <name> --strict`。任一失败则阻断提交。
#
# 动机：openspec delta 的语法错误只有在 /opsx:archive 流程里才会被捕获。
# 人手直接 git commit 会把坏 delta 推进 archive，导致后续 sync 崩。
#
# 触发条件（全部满足才跑校验）：
# 1. tool_name == "Bash"
# 2. tool_input.command 以 `git commit` 起头
# 3. staged 里至少有一个 openspec/changes/<name>/specs/**/*.md
#
# 其他 git commit 场景（如只改代码）原样放行。
set -euo pipefail

input=$(cat)
tool_name=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
if [[ "$tool_name" != "Bash" ]]; then
  exit 0
fi

command=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || true)
# 只对真正的 commit 触发——包括 git commit / git commit -m / HEREDOC 形式都以 "git commit" 起头
if [[ "$command" != git\ commit* ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# 找出 staged 的 delta spec 文件
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

#!/usr/bin/env bash
# PreToolUse hook: 当前分支为 main / master 时，硬阻断对源代码的 Edit / Write。
#
# 触发条件（全部满足才拦）：
# 1. settings.json matcher 已限制 tool_name ∈ {Edit, Write, MultiEdit, NotebookEdit}
# 2. 当前分支 = main 或 master（**直读 .git/HEAD 而非 git branch，省 ~80ms**）
# 3. file_path 在仓库内
# 4. 仓库内 file_path 不在白名单
#
# 白名单：CLAUDE.md / README.md / LICENSE / justfile / .claude/ / .github/ / docs/ / openspec/changes/
#
# 性能预算（见 .claude/rules/hooks-performance.md）：
# - feature 分支 99% 场景：.git/HEAD 直读后 case 放行，~5ms
# - main 分支：jq 提取 file_path + 白名单匹配，~30ms
set -euo pipefail

input=$(</dev/stdin)

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"

# 直读 .git/HEAD 拿 branch（bash 内置 read ~0ms vs `git branch --show-current` ~80ms vs `sed|head` ~20ms）
# 处理 worktree 场景：.git 是 file 时 follow gitdir
git_path="$project_dir/.git"
if [[ -f "$git_path" ]]; then
  # worktree: .git 是文件，单行 "gitdir: <path>"——bash 内置 read 替代 sed|head
  read -r line < "$git_path"
  head_file="${line#gitdir: }/HEAD"
elif [[ -d "$git_path" ]]; then
  head_file="$git_path/HEAD"
else
  exit 0
fi

[[ -r "$head_file" ]] || exit 0

read -r head_content < "$head_file"
# HEAD 形如 "ref: refs/heads/main" 或 detached 时是 commit hash
case "$head_content" in
  "ref: refs/heads/main"|"ref: refs/heads/master") ;;
  *) exit 0 ;;
esac

# 严谨提取 file_path（jq 比 python3 快 ~2.5×，fallback sed）
file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

# 仓库外文件直接放行（~/.zshrc / /tmp/* / 其他项目等）
abs_root=$(cd "$project_dir" && pwd)
if [[ "$file_path" != "$abs_root"/* ]]; then
  exit 0
fi

# 白名单（以仓库根为相对前缀匹配）
rel="${file_path#"$abs_root"/}"

case "$rel" in
  CLAUDE.md|README.md|LICENSE|justfile)
    exit 0 ;;
  .claude/*|.github/*|docs/*)
    exit 0 ;;
  openspec/changes/*)
    exit 0 ;;
esac

# branch 是 main 还是 master（从 head_content 反推用于错误信息）
case "$head_content" in
  *main*) branch=main ;;
  *) branch=master ;;
esac

cat >&2 <<EOF
BLOCKED: refusing to Edit/Write on '$branch' branch.

File: $rel

CLAUDE.md 第 1 条硬约束：开新工作前先开 feature 分支，不直接在 $branch 上写代码。

修复：
  git checkout -b feat/<slug>      # 或 fix/<slug>

白名单（在 main 直接改不会触发拦截）：
  - CLAUDE.md / README.md / LICENSE / justfile
  - .claude/ / .github/ / docs/
  - openspec/changes/<slug>/...（propose / apply 阶段允许）
  - 仓库外文件（~/.zshrc / /tmp/* 等）

如确需在 main 编辑该文件，请明确告诉用户原因并请求授权。
EOF
exit 2

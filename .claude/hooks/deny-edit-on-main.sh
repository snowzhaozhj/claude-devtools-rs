#!/usr/bin/env bash
# PreToolUse hook: 当前分支为 main / master 时，硬阻断对源代码的 Edit / Write。
#
# 动机：CLAUDE.md 第 1 条硬约束"开新工作前先 git checkout -b feat/<slug>，不要
# 直接在 main 上写代码"——但 git commit 阶段才拦截已经晚了（代码已改完，回滚成本
# 已发生）。把拦截前移到 Edit/Write 阶段。
#
# 触发条件（全部满足才拦）：
# 1. tool_name ∈ {Edit, Write, MultiEdit, NotebookEdit}
# 2. 当前分支 = main 或 master
# 3. file_path 不在白名单（白名单：CLAUDE.md / README.md / .github/ /
#    .claude/ / openspec/changes/ 内的非 archive 内容——这些"维护类"改动
#    在 main 直接做不算违规）
#
# 失败：exit 2，stderr 提示切分支命令。
set -euo pipefail

input=$(cat)

tool_name=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
case "$tool_name" in
  Edit|Write|MultiEdit|NotebookEdit) ;;
  *) exit 0 ;;
esac

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# 不在 git 仓库，放行
branch=$(git branch --show-current 2>/dev/null || true)
case "$branch" in
  main|master) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)

# 白名单（以仓库根为相对前缀匹配）：维护类改动在 main 上允许
abs_root=$(cd "$project_dir" && pwd)
rel="${file_path#"$abs_root"/}"

case "$rel" in
  CLAUDE.md|README.md|LICENSE|justfile)
    exit 0 ;;
  .claude/*|.github/*|docs/*)
    exit 0 ;;
  openspec/changes/*)
    # openspec/changes/*（含 propose 阶段的 scaffold）允许；openspec/specs/ 主 spec
    # 走 archive 写，不走人手 Edit——继续往下检查（命中 deny）
    exit 0 ;;
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

如确需在 main 编辑该文件，请明确告诉用户原因并请求授权。
EOF
exit 2

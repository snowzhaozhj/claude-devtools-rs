#!/usr/bin/env bash
# PreToolUse hook on Bash: 当 `git commit` 的消息是 `fix(...)` / `fix:` 开头但 staged
# 里**没有任何测试文件**时，stderr 警告但**不阻断**（exit 0）。
#
# 动机：codex 二审与 .claude/rules/codex-usage.md 都明确"fix 提交 SHALL 含回归测试"。
# 历史上多次出现"修了 bug 没补单测，下次回归"——本 hook 在 commit 前给一道提醒。
#
# 触发条件（全部满足才警告）：
# 1. tool_name == "Bash"
# 2. tool_input.command 形如 `git commit ...`
# 3. command 文本含 `fix(...)` 或 `fix:`（无论 -m "..." 还是 HEREDOC 形式）
# 4. staged 列表无任何匹配 *test*.rs / *.test.ts / *.spec.ts / *.test.svelte.ts
#
# **不阻断**——只在 stderr 写一段警告（exit 0）。Claude 看到后自行判断是否补单测。
#
# 关于 amend / rebase / cherry-pick：
# - `git commit --amend`：fix amend 可能只是改文案不动代码，警告可能 false positive；
#   不阻断 = 容忍 false positive，让人/Claude 看完决定
# - `git rebase` / `git cherry-pick`：内部进程产 commit，不通过 Bash 调，不命中
set -euo pipefail

input=$(cat)

tool_name=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
if [[ "$tool_name" != "Bash" ]]; then
  exit 0
fi

command=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || true)

if ! [[ "$command" =~ (^|[[:space:]]|;|&&|\|\|)git[[:space:]]+commit([[:space:]]|$) ]]; then
  exit 0
fi

if [[ "$command" =~ git[[:space:]]+commit[[:space:]]+(--help|-h)([[:space:]]|$) ]]; then
  exit 0
fi

# 命中 fix( / fix: **作为 commit message 起头** 的情形。
# 历史 bug：仅匹配子串会把 `feat: rename fix() helper` / `docs: explain fix:` 等
# 含 fix 子串的非-fix commit 误命中。锚定到 -m "fix( / -m "fix: / -m 'fix( / -m 'fix:
# 才是真正的 conventional-commits fix-prefix。
#
# HEREDOC 形式 `git commit -m "$(cat <<'EOF' ... fix(xxx): ... EOF)"`：
# command 文本里 fix( 出现在 EOF 块首行；锚定 "\nfix(" 或 EOF 后紧跟 fix
# 在简单匹配里难做且不常见——HEREDOC 形式 fix commit 这里**漏报**而非误报，可接受。
if ! [[ "$command" =~ -m[[:space:]]+[\'\"]fix[\(:] ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# `git diff --cached` 拿当前 staged。注意：`git commit -a` / `git commit <pathspec>`
# 等会把额外文件加进 commit——本 hook 只看已 staged 部分，对 `-a` 场景会有 false
# negative。**warn-only** 的代价可接受；阻断式 hook 才需精确。
staged=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null || true)

if [[ -z "$staged" ]]; then
  # 没有 staged 文件——可能 `--amend` 改文案或 `commit --allow-empty`，跳过警告
  exit 0
fi

# 匹配测试文件：
#   - 含 "test" 的 .rs 文件（覆盖 foo_test.rs / tests/xxx.rs / mod_test.rs）
#   - *.test.ts / *.test.svelte.ts / *.spec.ts
test_files=$(printf '%s\n' "$staged" | grep -E '(.*test.*\.rs$|.*\.test\.ts$|.*\.test\.svelte\.ts$|.*\.spec\.ts$)' || true)

if [[ -n "$test_files" ]]; then
  exit 0
fi

{
  echo "[warn-bare-fix] WARN: 检测到 fix(...) commit 但 staged 列表无测试文件。"
  echo "  Staged 文件："
  printf '    %s\n' $staged
  echo
  echo "  .claude/rules/codex-usage.md 指出：fix 提交 SHALL 含回归测试。"
  echo "  匹配模式：*test*.rs / *.test.ts / *.spec.ts / *.test.svelte.ts"
  echo "  本次仅警告不阻塞；若 bug 不可测（构建脚本 / 文档 / CI 配置）忽略此提示即可。"
} >&2

exit 0

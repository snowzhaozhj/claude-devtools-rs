#!/usr/bin/env bash
# PreToolUse hook on Bash: 在 `git commit` 之前跑 `cargo clippy --workspace --all-targets`，
# 失败则阻断 commit。
#
# 动机：PostToolUse `.rs` 单文件 clippy hook 只校验"被改的那个 crate"；跨 crate 的
# `pub use` 改动 / workspace-level lint 默认配置变化 等只有 workspace 级 clippy 才发现。
# 落到 commit 前一刻拦一道，避免"本地全绿但 PR CI 红"。
#
# 触发条件（全部满足才跑）：
# 1. tool_name == "Bash"
# 2. tool_input.command 形如 `git commit ...`
# 3. 排除：`git commit --help` / `git commit -h`（只是查文档，不实际产 commit）
#
# 关于 amend / rebase / cherry-pick：
# - `git commit --amend` 仍触发——amend 也是产新 commit，clippy 仍应校验
# - `git rebase` / `git cherry-pick` 由 git 内部进程产 commit，不通过 Bash tool 调，
#   故不会被本 hook 命中（设计上正确）
# - rebase 期间 conflict 解决后用户显式 `git commit` 时仍触发——合理
#
# 性能：clippy --workspace 在干净 cargo cache 下 ~30s；本机日常 < 5s（增量缓存命中）。
# 失败时 stderr 输出尾 50 行 + 引导，exit 2 阻断。
set -euo pipefail

input=$(cat)

tool_name=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
if [[ "$tool_name" != "Bash" ]]; then
  exit 0
fi

command=$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || true)

# 严格匹配 "git commit"（后跟空白或行尾），避免误命中 `git commit-tree`（plumbing 命令）
# 起始锚点：行首 / 空白 / `;` / `&&` / `||` —— 兼容复合命令
if ! [[ "$command" =~ (^|[[:space:]]|;|&&|\|\|)git[[:space:]]+commit([[:space:]]|$) ]]; then
  exit 0
fi

# `git commit --help` / `-h` 只是查文档，不产 commit，跳过
if [[ "$command" =~ git[[:space:]]+commit[[:space:]]+(--help|-h)([[:space:]]|$) ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# 没装 cargo（极少见，例如纯 docs PR 仓库）直接放行
if ! command -v cargo >/dev/null 2>&1; then
  exit 0
fi

log_file=$(mktemp)
# 失败时把日志移到 /tmp/cdt-clippy-fail-<pid>.log 保留给用户排查，
# 成功时清理。trap 只兜底失败-退出路径（如 cargo crash）防泄漏临时文件。
persist_log="/tmp/cdt-clippy-fail-$$.log"
trap 'rm -f "$log_file"' EXIT

if cargo clippy --workspace --all-targets -- -D warnings >"$log_file" 2>&1; then
  exit 0
fi

# 把日志固化到 persist_log，trap 仍清理原 mktemp 文件（已被 cp 走）
cp "$log_file" "$persist_log" 2>/dev/null || true

{
  echo "[workspace-clippy-pre-commit] 阻塞 git commit：cargo clippy --workspace 报错。"
  echo
  echo "===== 输出尾 50 行 ====="
  tail -50 "$log_file"
  echo "========================"
  echo
  echo "修复 clippy warning 后重新提交。完整日志：$persist_log（已固化，下次 hook 触发会被覆盖）"
  echo "（注：本 hook 只在 Claude 显式跑 git commit 时触发；git rebase / cherry-pick 不命中。）"
} >&2

exit 2

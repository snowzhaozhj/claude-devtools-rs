#!/usr/bin/env bash
# PostToolUse hook: 编辑 spec.md 后跑 spec purity ratchet check
# 让六类反模式 + ratchet 拦截在编辑当场暴露，避免 push 后才被 CI 红
#
# 仅触发：openspec/specs/<cap>/spec.md  与  openspec/changes/<slug>/specs/<cap>/spec.md
# 跳过：openspec/changes/archive/**（历史快照冻结）
#
# 性能预算：99% 命中（非 spec.md 编辑）case 预判 exit 0，~5ms
# 触发 spec.md 编辑：跑 check-spec-purity.sh ~150ms（30 个 spec grep 扫描）
#
# 不 block 编辑：检测到违规仅 stderr 警告 + 提示 --report 看命中，让作者继续
# Edit 但下一步 commit 前知道有问题。CI 才是硬拦截。
set -euo pipefail

input=$(</dev/stdin)

# 快速预判：file_path 不含 spec.md 直接放行
case "$input" in
  *'spec.md"'*) ;;
  *) exit 0 ;;
esac

file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ -z "$file_path" ]]; then
  exit 0
fi

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
rel="${file_path#"$project_dir/"}"

# 仅 openspec/specs/* 与 openspec/changes/<slug>/specs/* 触发；archive 跳过
case "$rel" in
  openspec/specs/*/spec.md|openspec/changes/*/specs/*/spec.md) ;;
  *) exit 0 ;;
esac
case "$rel" in
  openspec/changes/archive/*) exit 0 ;;
esac

cd "$project_dir"
if [[ ! -x scripts/check-spec-purity.sh ]]; then
  exit 0
fi

# stderr 警告但不 block；本地诊断绕过 baseline 下降 fail
SPEC_PURITY_ALLOW_DECREASE=1 bash scripts/check-spec-purity.sh >/dev/null 2>/tmp/spec-purity-after-edit.$$
rc=$?
if [[ "$rc" -ne 0 ]]; then
  {
    echo ""
    echo "⚠ spec purity 警告（编辑 $rel 后）"
    cat /tmp/spec-purity-after-edit.$$
    echo ""
    echo "看命中行：bash scripts/check-spec-purity.sh --report"
  } >&2
fi
rm -f /tmp/spec-purity-after-edit.$$
exit 0

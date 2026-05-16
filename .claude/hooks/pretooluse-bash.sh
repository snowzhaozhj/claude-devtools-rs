#!/usr/bin/env bash
# 合并 PreToolUse Bash hook：原 validate-openspec-before-commit + release-tail-check 两段业务。
# 合并理由：每个 Bash 工具调用串行跑 N 个 hook = N × bash 启动开销（每个 ~56ms）。
# 同 matcher 多 hook 合 1 个内部分流后省 (N-1) × 56ms / 调用。详见 .claude/rules/perf.md "Hook 性能"。
#
# 业务分流：
# - command 含 "git commit"  → openspec validate（对 staged delta spec 跑 --strict）
# - command 含 "git push"     → release-tail-check（拦截已完成但未 archive 的 change）
#
# 性能：99% Bash 调用通过 case 预判 exit 0，~56ms / 调用（原来 2 个 hook ~112ms）
set -euo pipefail

input=$(</dev/stdin)

# 快速分流：粗 grep 命令模式
mode=""
case "$input" in
  *'"command"'*'git commit'*) mode="commit" ;;
  *'"command"'*'git push'*)   mode="push" ;;
  *) exit 0 ;;
esac

# 严谨提取 command（jq 失败 fallback sed）
command=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# ---------------------------------------------------------------------------
# 分支 A: git commit → openspec validate
# ---------------------------------------------------------------------------
if [[ "$mode" == "commit" ]]; then
  if [[ "$command" != git\ commit* ]]; then
    exit 0
  fi

  staged=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null || true)
  [[ -n "$staged" ]] || exit 0

  changes=$(printf '%s\n' "$staged" \
    | grep -E '^openspec/changes/[^/]+/specs/.+\.md$' \
    | cut -d/ -f3 \
    | sort -u || true)
  [[ -n "$changes" ]] || exit 0

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
fi

# ---------------------------------------------------------------------------
# 分支 B: git push → release-tail-check
# ---------------------------------------------------------------------------
if [[ "$mode" == "push" ]]; then
  # 单词边界精确匹配 git push（避免误匹配 git push-config 等）
  if ! [[ "$command" =~ (^|[[:space:]&\;|])git[[:space:]]+push([[:space:]&\;|]|$) ]]; then
    exit 0
  fi

  command -v openspec >/dev/null 2>&1 || exit 0

  completed=$(openspec list --json 2>/dev/null \
    | jq -r '.changes[] | select(.status == "complete" or (.totalTasks != null and .completedTasks == .totalTasks)) | .name' \
    2>/dev/null || true)
  [[ -n "$completed" ]] || exit 0

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
fi

exit 0

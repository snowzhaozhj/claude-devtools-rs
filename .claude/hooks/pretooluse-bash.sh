#!/usr/bin/env bash
# 合并 PreToolUse Bash hook：原 validate-openspec-before-commit + release-tail-check 两段业务。
# 合并理由：每个 Bash 工具调用串行跑 N 个 hook = N × bash 启动开销（每个 ~56ms）。
# 同 matcher 多 hook 合 1 个内部分流后省 (N-1) × 56ms / 调用。详见 .claude/rules/perf.md "Hook 性能"。
#
# 业务分流（**非互斥**：命令可能同时含 commit 和 push 字符串，两段独立判定）：
# - command 含 "git push"     → release-tail-check（拦截已完成但未 archive 的 change）— 优先级高
# - command 起头是 "git commit" → openspec validate（对 staged delta spec 跑 --strict）
#
# 性能：99% Bash 调用通过 case 预判全跳过，~56ms / 调用（原来 2 个 hook ~112ms）
set -euo pipefail

input=$(</dev/stdin)

# 双预判：分别看 input 是否含 git push / git commit 模式（互不影响）
has_push=0
has_commit=0
case "$input" in
  *'"command"'*'git push'*) has_push=1 ;;
esac
case "$input" in
  *'"command"'*'git commit'*) has_commit=1 ;;
esac

# 都不命中直接放行
if (( has_push == 0 && has_commit == 0 )); then
  exit 0
fi

# 严谨提取 command（jq 失败 fallback sed）
command=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

# ---------------------------------------------------------------------------
# 分支 A（优先）: git push → release-tail-check
# 单词边界精确匹配；放在 commit 前因为 push 是更严重的 shared-state 操作，应优先拦截
# ---------------------------------------------------------------------------
if (( has_push )); then
  if [[ "$command" =~ (^|[[:space:]&\;|])git[[:space:]]+push([[:space:]&\;|]|$) ]]; then
    if command -v openspec >/dev/null 2>&1; then
      completed=$(openspec list --json 2>/dev/null \
        | jq -r '.changes[] | select(.status == "complete" or (.totalTasks != null and .completedTasks == .totalTasks)) | .name' \
        2>/dev/null || true)

      if [[ -n "$completed" ]]; then
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
    fi
  fi
fi

# ---------------------------------------------------------------------------
# 分支 B: git commit → openspec validate
# ---------------------------------------------------------------------------
if (( has_commit )); then
  if [[ "$command" == git\ commit* ]]; then
    staged=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null || true)
    if [[ -n "$staged" ]]; then
      changes=$(printf '%s\n' "$staged" \
        | grep -E '^openspec/changes/[^/]+/specs/.+\.md$' \
        | cut -d/ -f3 \
        | sort -u || true)

      if [[ -n "$changes" ]]; then
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
      fi
    fi
  fi
fi

exit 0

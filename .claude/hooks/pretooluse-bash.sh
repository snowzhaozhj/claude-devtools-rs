#!/usr/bin/env bash
# 合并 PreToolUse Bash hook：四段业务串行内部分流。
# 合并理由：每个 Bash 工具调用串行跑 N 个 hook = N × bash 启动开销（每个 ~56ms）。
# 同 matcher 多 hook 合 1 个内部分流后省 (N-1) × 56ms / 调用。详见 .claude/rules/perf.md "Hook 性能"。
#
# 业务分流（**非互斥**：命令可能同时含 commit 和 push 字符串，两段独立判定）：
# - command 含 "git push"      → release-tail-check（拦截已完成但未 archive 的 change）— 优先级高
# - command 起头是 "git commit"，按顺序：
#     1) openspec validate（对 staged delta spec 跑 --strict）—— 最快，失败即拒
#     2) workspace clippy（cargo clippy --workspace --all-targets -- -D warnings）—— 重，30s+
#     3) warn-bare-fix（fix(...) commit 无 test 文件时 stderr 警告，不阻断）—— 最轻
#
# 性能：99% Bash 调用通过 case 预判全跳过，~56ms / 调用（原来 4 个独立 hook ~224ms）
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
# 分支 B: git commit → openspec validate + workspace clippy + warn-bare-fix
# 排除 `git commit --help` / `-h`（查文档不产 commit）
# ---------------------------------------------------------------------------
if (( has_commit )) && [[ "$command" == git\ commit* ]] \
   && ! [[ "$command" =~ git[[:space:]]+commit[[:space:]]+(--help|-h)([[:space:]]|$) ]]; then

  staged=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null || true)

  # ---- B.1 openspec validate (staged delta spec → --strict) -----------------
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

  # ---- B.2 workspace clippy (cargo clippy --workspace --all-targets) --------
  # commit 前最后一道 lint：跨 crate `pub use` / workspace-level lint 改动只有
  # workspace 级 clippy 才发现；PostToolUse 单 crate clippy 抓不到。
  # 慢路径（30s+ cold cache / <5s warm），但通过早期 case 预判保证只在 git commit
  # 时触发，普通 Bash 调用 0 fork 跳过。
  if command -v cargo >/dev/null 2>&1; then
    clippy_log=$(mktemp)
    persist_log="/tmp/cdt-clippy-fail.log"
    if ! cargo clippy --workspace --all-targets -- -D warnings >"$clippy_log" 2>&1; then
      cp "$clippy_log" "$persist_log" 2>/dev/null || true
      {
        echo "[workspace-clippy-pre-commit] 阻塞 git commit：cargo clippy --workspace 报错。"
        echo
        echo "===== 输出尾 50 行 ====="
        tail -50 "$clippy_log"
        echo "========================"
        echo
        echo "修复 clippy warning 后重新提交。完整日志：$persist_log（每次失败覆盖，仅保留最新）"
      } >&2
      rm -f "$clippy_log"
      exit 2
    fi
    rm -f "$clippy_log"
  fi

  # ---- B.3 warn-bare-fix (fix(...) commit 无 test → stderr 警告，不阻断) -----
  # regex 锚定 -m "fix( / -m "fix: / -m 'fix( / -m 'fix: 起头，避免 `feat: ... fix()` 误命中。
  # HEREDOC 形式 fix commit 是已知漏报（warn-only 容忍）。
  if [[ "$command" =~ -m[[:space:]]+[\'\"]fix[\(:] ]] && [[ -n "$staged" ]]; then
    if ! printf '%s\n' "$staged" | grep -qE '(.*test.*\.rs$|.*\.test\.ts$|.*\.test\.svelte\.ts$|.*\.spec\.ts$)'; then
      {
        echo "[warn-bare-fix] WARN: 检测到 fix(...) commit 但 staged 列表无测试文件。"
        echo "  Staged 文件："
        printf '    %s\n' $staged
        echo
        echo "  .claude/rules/codex-usage.md 指出：fix 提交 SHALL 含回归测试。"
        echo "  匹配模式：*test*.rs / *.test.ts / *.spec.ts / *.test.svelte.ts"
        echo "  本次仅警告不阻塞；若 bug 不可测（构建脚本 / 文档 / CI 配置）忽略此提示即可。"
      } >&2
    fi
  fi
fi

exit 0

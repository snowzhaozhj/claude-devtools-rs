#!/usr/bin/env bash
# 半自动 worktree 清理：扫 git worktree list，对每个非主 repo 的 worktree
# 检查 PR 状态 + uncommitted/unpushed，列出可安全删除的候选。
#
# 用法：
#   bash scripts/clean-worktrees.sh           # dry-run（默认）
#   bash scripts/clean-worktrees.sh --apply   # 真删 worktree + 本地分支

set -euo pipefail

APPLY=false
case "${1:-}" in
  --apply) APPLY=true ;;
  -h|--help)
    sed -n '2,9p' "$0"
    exit 0
    ;;
  '') ;;
  *)
    echo "未知参数：$1（用 --help 看用法）" >&2
    exit 2
    ;;
esac

DEFAULT_BRANCH="main"
COMMON_DIR="$(git rev-parse --git-common-dir)"
MAIN_ROOT="$(cd "$(dirname "$COMMON_DIR")" && pwd)"
CURRENT_WT="$(git rev-parse --show-toplevel)"

have_gh=true
if ! command -v gh >/dev/null 2>&1; then
  have_gh=false
  echo "⚠️  gh CLI 未安装，无法查 PR 状态——只看 uncommitted/unpushed" >&2
fi

# 收集所有 worktree（跳过主 repo）
worktrees=()
while IFS= read -r line; do
  case "$line" in
    "worktree "*)
      path="${line#worktree }"
      if [[ "$path" != "$MAIN_ROOT" ]]; then
        worktrees+=("$path")
      fi
      ;;
  esac
done < <(git worktree list --porcelain)

if [[ ${#worktrees[@]} -eq 0 ]]; then
  echo "无 worktree 可清理"
  exit 0
fi

echo "=== 扫描 ${#worktrees[@]} 个 worktree ==="
echo ""

safe_remove=()
needs_review=()
active=()
skipped=()
total_kb=0

for wt in "${worktrees[@]}"; do
  name="$(basename "$wt")"

  if [[ "$wt" == "$CURRENT_WT" ]]; then
    skipped+=("$name|当前会话所在 worktree")
    continue
  fi

  branch="$(git -C "$wt" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '')"
  if [[ -z "$branch" || "$branch" == "HEAD" ]]; then
    skipped+=("$name|detached HEAD（不删除以防丢 commit）")
    continue
  fi

  size_h="$(command du -sh "$wt" 2>/dev/null | awk '{print $1}')"
  size_kb="$(command du -sk "$wt" 2>/dev/null | awk '{print $1}')"

  uncommitted=$(git -C "$wt" status --porcelain 2>/dev/null | wc -l | tr -d ' ')

  # 未 push commits：优先比 upstream，没 upstream 比 main
  unpushed=0
  if git -C "$wt" rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
    unpushed=$(git -C "$wt" log '@{u}..HEAD' --oneline 2>/dev/null | wc -l | tr -d ' ')
  else
    unpushed=$(git -C "$wt" log "${DEFAULT_BRANCH}..HEAD" --oneline 2>/dev/null | wc -l | tr -d ' ')
  fi

  # PR 状态
  pr_state="unknown"
  if [[ "$have_gh" == "true" ]]; then
    pr_json=$(gh pr list --head "$branch" --state all --json number,state --limit 1 2>/dev/null || echo '[]')
    if [[ "$pr_json" != "[]" && -n "$pr_json" ]]; then
      pr_state=$(printf '%s' "$pr_json" | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d[0]["state"].lower() if d else "none")' 2>/dev/null || echo "unknown")
    else
      pr_state="none"
    fi
  fi

  if [[ "$pr_state" == "merged" && "$uncommitted" == "0" && "$unpushed" == "0" ]]; then
    safe_remove+=("$wt|$name|$branch|$size_h|$size_kb")
    total_kb=$((total_kb + size_kb))
  elif [[ "$pr_state" == "merged" ]]; then
    needs_review+=("$name|$size_h|merged 但 uncommitted=$uncommitted unpushed=$unpushed")
  elif [[ "$pr_state" == "closed" && "$uncommitted" == "0" && "$unpushed" == "0" ]]; then
    safe_remove+=("$wt|$name|$branch|$size_h|$size_kb")
    total_kb=$((total_kb + size_kb))
  else
    active+=("$name|$size_h|PR=$pr_state uncommitted=$uncommitted unpushed=$unpushed")
  fi
done

# 报告
if [[ ${#safe_remove[@]} -gt 0 ]]; then
  echo "=== ✅ 可安全删除（PR merged/closed + 工作树干净）==="
  for entry in "${safe_remove[@]}"; do
    IFS='|' read -r _ name branch size_h _ <<< "$entry"
    printf "  %-50s %8s  branch: %s\n" "$name" "$size_h" "$branch"
  done
  recoverable_mb=$((total_kb / 1024))
  echo ""
  printf "  共 %d 个，预计释放 %d MB\n" "${#safe_remove[@]}" "$recoverable_mb"
  echo ""
fi

if [[ ${#needs_review[@]} -gt 0 ]]; then
  echo "=== ⚠️  merged 但有改动（需手动确认）==="
  for entry in "${needs_review[@]}"; do
    IFS='|' read -r name size_h reason <<< "$entry"
    printf "  %-50s %8s  %s\n" "$name" "$size_h" "$reason"
  done
  echo ""
fi

if [[ ${#active[@]} -gt 0 ]]; then
  echo "=== ▶ 活跃中（保留）==="
  for entry in "${active[@]}"; do
    IFS='|' read -r name size_h reason <<< "$entry"
    printf "  %-50s %8s  %s\n" "$name" "$size_h" "$reason"
  done
  echo ""
fi

if [[ ${#skipped[@]} -gt 0 ]]; then
  echo "=== ⏭  跳过 ==="
  for entry in "${skipped[@]}"; do
    IFS='|' read -r name reason <<< "$entry"
    printf "  %-50s %s\n" "$name" "$reason"
  done
  echo ""
fi

# Apply
if [[ "$APPLY" == "true" && ${#safe_remove[@]} -gt 0 ]]; then
  echo "=== 执行删除 ==="
  for entry in "${safe_remove[@]}"; do
    IFS='|' read -r wt name branch _ _ <<< "$entry"
    echo "→ git worktree remove $name"
    if git worktree remove "$wt" 2>&1 | sed 's/^/    /'; then
      :
    else
      echo "    ❌ 失败，跳过分支删除"
      continue
    fi
    if git show-ref --verify --quiet "refs/heads/$branch"; then
      echo "→ git branch -D $branch"
      git branch -D "$branch" 2>&1 | sed 's/^/    /' || true
    fi
  done
  echo ""
  git worktree prune
  echo "✅ 完成；释放空间约 $((total_kb / 1024)) MB"
elif [[ ${#safe_remove[@]} -gt 0 ]]; then
  echo "→ 加 --apply 执行真删：bash scripts/clean-worktrees.sh --apply"
elif [[ ${#needs_review[@]} -eq 0 && ${#active[@]} -eq 0 ]]; then
  echo "✓ 没有可清理的 worktree"
else
  echo "✓ 没有可自动清理的 worktree（活跃 / 需手动处理的见上方列表）"
fi

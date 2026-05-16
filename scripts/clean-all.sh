#!/usr/bin/env bash
# 一键清理：已 merged worktree（整体删）+ 主仓 cargo target + 活跃 worktree 里的 cargo target
#
# 用法：
#   bash scripts/clean-all.sh           # dry-run，列候选 + 预计释放空间（默认）
#   bash scripts/clean-all.sh --apply   # 真删
#
# 清理范围：
#   (1) 已 merged worktree（复用 clean-worktrees.sh）—— PR merged/closed && 工作树干净
#   (2) 主仓 ./target + ./src-tauri/target —— cargo workspace 缓存（下次需重编译）
#   (3) 活跃 worktree 的 target + src-tauri/target —— 保留 worktree 代码，仅清缓存
#
# 注意：当前 cwd 所在 worktree 的 cargo target 也会被清——这是有意为之，
# 用户在用的 worktree 本来就该能"清缓存重新编译"。

set -euo pipefail

APPLY=false
case "${1:-}" in
  --apply) APPLY=true ;;
  -h|--help)
    sed -n '2,17p' "$0"
    exit 0
    ;;
  '') ;;
  *)
    echo "未知参数：$1（用 --help 看用法）" >&2
    exit 2
    ;;
esac

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMMON_DIR="$(git rev-parse --git-common-dir)"
MAIN_ROOT="$(cd "$(dirname "$COMMON_DIR")" && pwd)"

hr() { command du -sh "$1" 2>/dev/null | awk '{print $1}'; }
kb() { command du -sk "$1" 2>/dev/null | awk '{print $1}'; }

# ──────── (1) merged worktree ────────
echo "=== (1) 已 merged worktree（PR merged/closed + 工作树干净）==="
echo ""
bash "$SCRIPT_DIR/clean-worktrees.sh"
echo ""

# 拿到将被整体删除的 worktree 路径列表，cargo target 扫描时跳过避免重复算
removable_wts_raw="$(bash "$SCRIPT_DIR/clean-worktrees.sh" --list-removable 2>/dev/null || true)"
declare -a removable_wts=()
if [[ -n "$removable_wts_raw" ]]; then
  while IFS= read -r line; do
    [[ -n "$line" ]] && removable_wts+=("$line")
  done <<< "$removable_wts_raw"
fi

is_removable_wt() {
  local target="$1"
  local wt
  for wt in "${removable_wts[@]}"; do
    [[ "$wt" == "$target" ]] && return 0
  done
  return 1
}

# ──────── (2) + (3) cargo target ────────
echo "=== (2) 主仓 cargo target ==="
echo ""
declare -a target_paths=()
total_kb=0

for sub in "target" "src-tauri/target"; do
  p="$MAIN_ROOT/$sub"
  if [[ -d "$p" ]]; then
    size_h="$(hr "$p")"
    size_kb="$(kb "$p")"
    printf "  %-50s %10s\n" "$sub" "$size_h"
    target_paths+=("$p")
    total_kb=$((total_kb + size_kb))
  fi
done
echo ""

echo "=== (3) 活跃 worktree 的 cargo target（merged 的已在 (1) 一并删，此处跳过）==="
echo ""
active_target_count=0
while IFS= read -r line; do
  case "$line" in
    "worktree "*)
      wt="${line#worktree }"
      [[ "$wt" == "$MAIN_ROOT" ]] && continue
      if is_removable_wt "$wt"; then
        continue
      fi
      name="$(basename "$wt")"
      for sub in "target" "src-tauri/target"; do
        p="$wt/$sub"
        if [[ -d "$p" ]]; then
          size_h="$(hr "$p")"
          size_kb="$(kb "$p")"
          printf "  %-50s %10s\n" "$name/$sub" "$size_h"
          target_paths+=("$p")
          total_kb=$((total_kb + size_kb))
          active_target_count=$((active_target_count + 1))
        fi
      done
      ;;
  esac
done < <(git worktree list --porcelain)

if [[ "$active_target_count" -eq 0 ]]; then
  echo "  (无)"
fi
echo ""

# ──────── 汇总 ────────
total_mb=$((total_kb / 1024))
echo "=== 汇总 ==="
printf "  cargo target 清理候选：%d 个目录，预计释放 %d MB（%.1f G）\n" \
  "${#target_paths[@]}" "$total_mb" "$(awk "BEGIN{print $total_mb/1024}")"
echo "  + 已 merged worktree 整体删（含其 target，见上方 (1) 报告）"
echo ""

# ──────── 执行 ────────
if [[ "$APPLY" == "true" ]]; then
  echo "=== 执行清理 ==="
  echo ""
  echo "→ (1) 删 merged worktree"
  bash "$SCRIPT_DIR/clean-worktrees.sh" --apply
  echo ""
  echo "→ (2)+(3) 删 cargo target"
  for p in "${target_paths[@]}"; do
    if [[ -d "$p" ]]; then
      echo "  rm -rf $p"
      rm -rf "$p"
    fi
  done
  echo ""
  printf "✅ 完成；cargo target 已释放约 %d MB（merged worktree 释放量见上方 (1) 报告）\n" "$total_mb"
else
  echo "→ 加 --apply 真删：bash scripts/clean-all.sh --apply"
  echo "  或：just clean-all-apply"
fi

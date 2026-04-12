#!/bin/bash
# port-all.sh — 自动推进剩余 capability port
#
# 用法:
#   bash scripts/port-all.sh
#
# 失败时会打印完整恢复指南，复制粘贴即可恢复。
# 如需跳过已完成的 capability，修改下方 CAPS 数组。

set -euo pipefail

CAPS=(
  file-watching
  session-search
  configuration-management
  notification-triggers
  team-coordination-metadata
  ssh-remote-context
  ipc-data-api
  http-data-api
)

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"

# ── 失败处理：打印当前状态 + 恢复指南 ──

fail() {
  local cap="$1" phase="$2" detail="$3"
  local tasks_done tasks_remaining last_commit
  tasks_done=$(grep -c '\[x\]' "openspec/changes/port-${cap}/tasks.md" 2>/dev/null || echo 0)
  tasks_remaining=$(grep -c '\[ \]' "openspec/changes/port-${cap}/tasks.md" 2>/dev/null || echo 0)
  last_commit=$(git log --oneline -1 2>/dev/null || echo "(no commits)")

  echo ""
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║  ❌ 失败: port-${cap} @ ${phase}"
  echo "╠══════════════════════════════════════════════════════════════╣"
  echo "║  原因: ${detail}"
  echo "╠══════════════════════════════════════════════════════════════╣"
  echo "║  当前状态:"
  echo "║    分支:       port/${cap}"
  echo "║    最后 commit: ${last_commit}"
  echo "║    tasks 进度:  ${tasks_done} done / ${tasks_remaining} remaining"
  echo "║"
  echo "║  恢复步骤:"

  case "$phase" in
    propose)
      cat <<GUIDE
║
║    # propose 失败，无代码改动，直接重跑：
║    git checkout port/${cap}
║    claude --model sonnet -p '/opsx:propose port-${cap}' --allowedTools '*'
║    git add -A && git commit -m 'port-${cap}: propose'
GUIDE
      ;;
    apply)
      cat <<GUIDE
║
║    # apply 中途失败，已有 commit 是安全的检查点
║
║    # 选项 A: 从断点继续（推荐）
║    git checkout port/${cap}
║    claude --model opus -p '对 port-${cap} 执行 /opsx:apply。跳过 tasks.md 已勾选的，从未完成的 task 继续。每完成一个 task section 通过 test + clippy 后立即 commit。' --allowedTools '*'
║
║    # 选项 B: 回退到最后一个完整 task，重试
║    git checkout port/${cap}
║    git reset --hard HEAD  # 丢弃未提交的半成品
║    claude --model opus -p '/opsx:apply port-${cap}' --allowedTools '*'
GUIDE
      ;;
    test)
      cat <<GUIDE
║
║    # 测试失败，代码已写完但有 bug
║    git checkout port/${cap}
║    cargo test --workspace 2>&1 | tail -30  # 先看哪个测试挂了
║    claude --model opus -p '修复 port-${cap} 的测试失败，运行 cargo test --workspace 查看错误并修复，每个修复单独 commit' --allowedTools '*'
GUIDE
      ;;
    clippy)
      cat <<GUIDE
║
║    # clippy 不通过，通常是小问题
║    git checkout port/${cap}
║    claude --model sonnet -p '修复所有 clippy warnings: cargo clippy --workspace --all-targets -- -D warnings，修完后 commit' --allowedTools '*'
GUIDE
      ;;
    review-fix)
      cat <<GUIDE
║
║    # review 修复引入了新问题
║    git checkout port/${cap}
║    git diff HEAD~1 --stat  # 看 review 改了什么
║
║    # 选项 A: 撤销 review 的修改，手动处理
║    git revert HEAD --no-edit
║
║    # 选项 B: 让 opus 修
║    claude --model opus -p '上一轮 review 修复引入了测试失败，诊断并修复' --allowedTools '*'
GUIDE
      ;;
    archive)
      cat <<GUIDE
║
║    # archive 失败，代码本身没问题
║    git checkout port/${cap}
║    claude --model sonnet -p '/opsx:archive port-${cap}' --allowedTools '*'
║    git add -A && git commit -m 'port-${cap}: archive'
GUIDE
      ;;
  esac

  # 打印恢复后继续的指南
  echo "║"
  echo "║  恢复后继续跑剩余 capabilities:"
  echo "║    git checkout main && git merge --no-ff port/${cap}"

  local found=false
  local remaining=()
  for c in "${CAPS[@]}"; do
    if [ "$found" = true ]; then
      remaining+=("$c")
    fi
    if [ "$c" = "$cap" ]; then
      found=true
    fi
  done

  if [ ${#remaining[@]} -gt 0 ]; then
    echo "║    # 然后修改脚本 CAPS，只保留剩余的："
    echo "║    # CAPS=( ${remaining[*]} )"
    echo "║    bash scripts/port-all.sh"
  else
    echo "║    # 这是最后一个 capability，修复后就完成了！"
  fi

  echo "╚══════════════════════════════════════════════════════════════╝"
  exit 1
}

# ── 主循环 ──

echo "🚀 开始自动 port，共 ${#CAPS[@]} 个 capability"
echo "   时间: $(date '+%Y-%m-%d %H:%M')"
echo ""

for cap in "${CAPS[@]}"; do
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "  [$(date +%H:%M)] 即将开始 port-${cap}"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  # 安全确认：给 5 秒窗口取消，避免无人值守时跑进付费额度
  read -t 5 -p "  继续? (5秒内按 Ctrl+C 取消，回车立即继续) " || true
  echo ""

  # 创建或切换到分支
  git checkout -b "port/${cap}" main 2>/dev/null \
    || git checkout "port/${cap}"

  # ── Phase 1: propose (sonnet) ──
  echo "[$(date +%H:%M)] Phase 1/6: propose (sonnet)"
  claude --model sonnet -p \
    "/opsx:propose port-${cap}" --allowedTools '*' \
    || fail "$cap" "propose" "claude propose 会话异常退出"
  git add -A && git commit -m "port-${cap}: propose" --allow-empty

  # ── Phase 2: apply (opus) ──
  echo "[$(date +%H:%M)] Phase 2/6: apply (opus)"
  claude --model opus -p \
    "对 port-${cap} 执行 /opsx:apply。每完成一个 task section 通过 test + clippy 后立即 commit。" \
    --allowedTools '*' \
    || fail "$cap" "apply" "claude apply 会话异常退出"

  # ── Phase 3: 硬卡点验证 ──
  echo "[$(date +%H:%M)] Phase 3/6: 全量验证"
  cargo test --workspace \
    || fail "$cap" "test" "cargo test 失败"
  cargo clippy --workspace --all-targets -- -D warnings \
    || fail "$cap" "clippy" "clippy warnings"

  # ── Phase 4: review + 自动修 (sonnet) ──
  echo "[$(date +%H:%M)] Phase 4/6: code review (sonnet)"
  claude --model sonnet -p \
    "Review port-${cap} 的改动(git diff main)。如果发现真正的 bug 或 spec 不符，直接修复并 commit。如果没有问题，什么都不做。" \
    --allowedTools '*'

  # ── Phase 5: review 修复后再验证 ──
  echo "[$(date +%H:%M)] Phase 5/6: review 后验证"
  cargo test --workspace \
    || fail "$cap" "review-fix" "review 修复后测试失败"

  # ── Phase 6: archive (sonnet) ──
  echo "[$(date +%H:%M)] Phase 6/6: archive (sonnet)"
  claude --model sonnet -p \
    "/opsx:archive port-${cap}" --allowedTools '*' \
    || fail "$cap" "archive" "claude archive 会话异常退出"
  git add -A && git commit -m "port-${cap}: archive" --allow-empty

  # ── 合回 main ──
  git checkout main && git merge --no-ff "port/${cap}" \
    -m "Merge port-${cap}: 完成 ${cap} capability port"
  echo ""
  echo "  ✅ [$(date +%H:%M)] port-${cap} 完成"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  🎉 全部 ${#CAPS[@]} 个 capability port 完成！"
echo "  时间: $(date '+%Y-%m-%d %H:%M')"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

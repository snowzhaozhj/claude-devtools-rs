#!/usr/bin/env bash
# release-bump.sh — 一步完成 bump 版本号 + 同步两份 Cargo.lock + 本地 commit
# 用法: just bump X.Y.Z（先在 chore/release-X.Y.Z 分支上跑）
#
# 这是发版流水线的"本地段"自动化——消除手动 sed 三处 / 手动 git add 5 文件 /
# 漏 release-check 让 lock 脱节（详 release-runbook F5）等踩坑。
# 后续 push / open PR / wait CI / merge / tag 仍由 Agent 串。

set -euo pipefail

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "❌ 用法: just bump X.Y.Z（如 just bump 0.5.6）" >&2
    exit 1
fi

# Windows MSI 硬约束：纯数字 X.Y.Z，不接受 -rc / -beta 等 pre-release 标记
# 详 release-runbook F2（`pre-release identifier must be numeric-only`）。
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "❌ 版本号必须是 X.Y.Z 纯数字（Windows MSI 限制，详 release-runbook F2）" >&2
    echo "   收到: $VERSION" >&2
    exit 1
fi

# 当前分支检查：禁止直接在 main / master 上 bump
CURRENT_BRANCH=$(git branch --show-current)
if [[ "$CURRENT_BRANCH" == "main" || "$CURRENT_BRANCH" == "master" ]]; then
    echo "❌ 不能在 $CURRENT_BRANCH 分支上 bump，先 git checkout -b chore/release-$VERSION" >&2
    exit 1
fi

# 工作树检查（已跟踪文件不能有未提交改动；未跟踪文件如 .claude/worktrees/ 允许）
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "❌ 工作树有未提交改动，先 stash 或 commit" >&2
    git status --short >&2
    exit 1
fi

# 探测当前版本（以 workspace Cargo.toml 为准）
CUR=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')

if [[ "$CUR" == "$VERSION" ]]; then
    echo "⚠️  版本号已是 $VERSION，跳过 sed，直接跑 release-check + commit"
else
    echo "→ bump 三处版本号: $CUR → $VERSION"
    sed -i.bak "s/^version = \"$CUR\"\$/version = \"$VERSION\"/" Cargo.toml src-tauri/Cargo.toml
    sed -i.bak "s/\"version\": \"$CUR\",/\"version\": \"$VERSION\",/" src-tauri/tauri.conf.json
    rm -f Cargo.toml.bak src-tauri/Cargo.toml.bak src-tauri/tauri.conf.json.bak
fi

echo "→ just release-check（会同步刷新两份 Cargo.lock + 跑 preflight）"
just release-check

# 如果本来就是 $VERSION 且 release-check 没动 lock，git add 仍然安全（5 个文件全在）
echo "→ git add 5 文件 + commit"
git add Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json

# 检查暂存区是否真有改动；release-check 后 lock 没动 + manifest 也没动 = 空 commit 拦截
if git diff --cached --quiet; then
    echo "⚠️  暂存区无改动（版本号已是 $VERSION 且 lock 未变），跳过 commit"
else
    git commit -m "chore(release): $VERSION"
fi

echo ""
echo "✅ 本地 bump + commit 完成（branch: $CURRENT_BRANCH）"
echo ""
echo "下一步："
echo "    git push -u origin $CURRENT_BRANCH"
echo "    gh pr create --title 'chore(release): $VERSION' --body ..."
echo "    等 CI 全绿 → merge → 在 main 上 git tag v$VERSION && git push origin v$VERSION"
echo ""
echo "tag push 后 release.yml 末尾的 publish job 会自动 verify 17 个 asset 完整性"
echo "然后 un-draft 发布——无需人工 gh release edit。"

#!/usr/bin/env bash
# 端到端发版脚本：bump → preflight → PR → wait-ci → merge → tag → 监控 release.yml → draft ready
#
# 用法:
#   scripts/release.sh X.Y.Z              # 真实发版
#   scripts/release.sh --dry-run X.Y.Z    # 不执行 destructive 操作（push/merge/tag/...）
#   scripts/release.sh --resume X.Y.Z     # 跳过已完成步骤（按当前 git 状态推断）
#
# 退出码:
#   0   draft ready，等用户/agent 决策 publish
#   1   入参错误 / 前置条件不满足（在 main 分支 / 工作树脏 / 版本回退 / 字母版本号）
#   2   preflight 失败（fmt/lint/test/spec-validate 任一红）
#   3   PR CI 红
#   4   release.yml 红（matrix race / runner 不可用 / minisign 链 / lock 不同步）
#   5   release.yml 通过但 asset 不齐
#   99  未知错误
#
# 设计原则：失败即停 + 给清晰下一步提示；不自动套需要改 workflow / secret 的 fix（F1/F3/F4），
# 只把诊断信息打到 stderr 让 agent 接管。

set -euo pipefail

# ────── 解析参数 ──────
DRY_RUN=0
RESUME=0
NEW_VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --resume) RESUME=1; shift ;;
    --help|-h)
      sed -n '2,18p' "$0"
      exit 0
      ;;
    -*) echo "未知 flag: $1" >&2; exit 1 ;;
    *)
      if [[ -n "${NEW_VERSION}" ]]; then
        echo "❌ 多个版本号参数（${NEW_VERSION} + $1），只能传一个" >&2
        exit 1
      fi
      NEW_VERSION="$1"; shift
      ;;
  esac
done

if [[ -z "$NEW_VERSION" ]]; then
  echo "用法: $0 [--dry-run] [--resume] X.Y.Z" >&2
  exit 1
fi

# 纯数字 SemVer 校验（F2: Windows MSI 不接受字母后缀）
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "❌ F2: 版本号必须纯数字 X.Y.Z（Windows MSI bundler 不接受 0.5.6-rc.1 / 0.5.6-beta 等）" >&2
  exit 1
fi

# ────── 工具函数 ──────

run() {
  # 在 dry-run 模式下，只 echo 不真正执行 destructive 命令
  echo "  $ $*"
  if [[ $DRY_RUN -eq 0 ]]; then
    "$@"
  fi
}

step() {
  echo ""
  echo "━━━━ $* ━━━━"
}

die() {
  local code="$1"; shift
  echo "" >&2
  echo "❌ $*" >&2
  exit "$code"
}

# 读取三处版本号
read_versions() {
  ROOT_VER=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
  TAURI_CARGO_VER=$(grep -E '^version\s*=' src-tauri/Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
  TAURI_CONF_VER=$(grep -E '"version":' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
}

# semver 比较：a > b 返回 0；否则非 0
semver_gt() {
  # a=$1 b=$2; 用 sort -V 简单实现
  [[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" == "$1" ]] && [[ "$1" != "$2" ]]
}

# ────── 切到仓库根 ──────
cd "$(git rev-parse --show-toplevel)"

# ────── Step 0: 前置检查 ──────
step "Step 0: 前置检查"

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
RELEASE_BRANCH="chore/release-${NEW_VERSION}"

# 在 main / master 上跑是合法路径——Step 1 会自动从最新 origin/main 切 release 分支。
# 仅当工作树脏才拦（下面统一处理）。

# 工作树检查（resume / dry-run 模式下放宽：dry-run 不真写文件，resume 续跑保留中间产物）
if [[ $RESUME -eq 0 && $DRY_RUN -eq 0 ]] && [[ -n "$(git status --porcelain)" ]]; then
  die 1 "工作树不干净，请先 commit / stash"
fi
if [[ $DRY_RUN -eq 1 ]] && [[ -n "$(git status --porcelain)" ]]; then
  echo "  ⚠️  工作树有改动（dry-run 不强求干净，但真跑前必须 commit / stash）"
fi

read_versions
echo "  当前版本: $ROOT_VER (workspace) / $TAURI_CARGO_VER (src-tauri) / $TAURI_CONF_VER (tauri.conf)"
echo "  目标版本: $NEW_VERSION"

if [[ "$ROOT_VER" != "$TAURI_CARGO_VER" || "$ROOT_VER" != "$TAURI_CONF_VER" ]]; then
  die 1 "三处版本号不一致，请先手动同步"
fi

if [[ "${ROOT_VER}" == "${NEW_VERSION}" ]]; then
  echo "  ⚠️  版本号已是 ${NEW_VERSION}，跳过 bump（resume 模式）"
  SKIP_BUMP=1
else
  SKIP_BUMP=0
  if ! semver_gt "${NEW_VERSION}" "${ROOT_VER}"; then
    die 1 "新版本 ${NEW_VERSION} 不大于当前 ${ROOT_VER}（不允许版本回退）"
  fi
fi

# 检查必要命令
for cmd in git gh cargo just sed grep; do
  command -v "$cmd" >/dev/null || die 1 "缺少命令: $cmd"
done

# fetch + 检查 origin/main 同步
echo "  fetch origin..."
run git fetch origin main

# ────── Step 1: 切分支 ──────
step "Step 1: 切分支 $RELEASE_BRANCH"

if [[ "$CURRENT_BRANCH" == "$RELEASE_BRANCH" ]]; then
  echo "  已在 ${RELEASE_BRANCH}，跳过"
elif git rev-parse --verify "$RELEASE_BRANCH" >/dev/null 2>&1; then
  if [[ $RESUME -eq 1 ]]; then
    run git checkout "$RELEASE_BRANCH"
  else
    die 1 "分支 $RELEASE_BRANCH 已存在；如需续跑请加 --resume"
  fi
else
  run git checkout -b "$RELEASE_BRANCH" origin/main
fi

# ────── Step 2: bump 三处版本号 ──────
step "Step 2: bump 三处版本号"

if [[ $SKIP_BUMP -eq 1 ]]; then
  echo "  已 bumped，跳过"
else
  # 严格 section 限定：仅替换 [workspace.package] / [package] section 下的 version =
  # 防止误中其它 [dependencies.X] / [profile.release] 等 section 里偶然出现的 version 字段。
  if [[ $DRY_RUN -eq 0 ]]; then
    # workspace Cargo.toml: 在 [workspace.package] 段后第一个 ^version = "..."
    NV="${NEW_VERSION}" perl -i -pe '
      BEGIN { $in=0; $done=0 }
      if (/^\[workspace\.package\]/) { $in=1; next }
      elsif (/^\[/) { $in=0 }
      if ($in && !$done && /^version\s*=\s*"[^"]+"/) {
        s/"[^"]+"/"$ENV{NV}"/;
        $done=1;
      }
    ' Cargo.toml
    # src-tauri/Cargo.toml: [package] 段下第一个 ^version = "..."
    NV="${NEW_VERSION}" perl -i -pe '
      BEGIN { $in=0; $done=0 }
      if (/^\[package\]/) { $in=1; next }
      elsif (/^\[/) { $in=0 }
      if ($in && !$done && /^version\s*=\s*"[^"]+"/) {
        s/"[^"]+"/"$ENV{NV}"/;
        $done=1;
      }
    ' src-tauri/Cargo.toml
    # tauri.conf.json: 顶层 "version": "..."（JSON 没 section 概念，但本仓只一个 version key）
    NV="${NEW_VERSION}" perl -i -pe 's/"version":\s*"[^"]+"/"version": "$ENV{NV}"/' src-tauri/tauri.conf.json
  fi
  echo "  ✓ 三处已改"

  # 验证
  read_versions
  if [[ $DRY_RUN -eq 0 ]]; then
    if [[ "$ROOT_VER" != "$NEW_VERSION" || "$TAURI_CARGO_VER" != "$NEW_VERSION" || "$TAURI_CONF_VER" != "$NEW_VERSION" ]]; then
      die 99 "bump 失败：实际读到 $ROOT_VER / $TAURI_CARGO_VER / $TAURI_CONF_VER"
    fi
  fi
fi

# ────── Step 3: 同步 lock + preflight ──────
step "Step 3: preflight (fmt/lint/test/spec-validate)，preflight 内部 cargo build 会顺带刷 Cargo.lock"

if [[ $DRY_RUN -eq 0 ]]; then
  if ! just preflight; then
    die 2 "preflight 失败；修完后用 --resume ${NEW_VERSION} 续跑"
  fi
else
  echo "  [dry-run] 跳过 just preflight"
fi

# ────── Step 4: commit + push ──────
step "Step 4: commit + push"

# 严格基于 release 三件套 + lockfile 检测改动；不 git add -A 避免误提交无关 staged 改动。
RELEASE_FILES=(Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json)
RELEASE_DIRTY=$(git status --porcelain -- "${RELEASE_FILES[@]}" | wc -l | tr -d ' ')
OTHER_DIRTY=$(git status --porcelain | grep -vE "^.. ($(IFS='|'; echo "${RELEASE_FILES[*]}" | sed 's,/,\\/,g'))$" | wc -l | tr -d ' ')

if [[ "$OTHER_DIRTY" -gt 0 ]]; then
  echo "  ⚠️  工作树有非 release 文件的改动（$OTHER_DIRTY 个），仅 commit release 三件套；其它请单独处理"
  if [[ $DRY_RUN -eq 0 && $RESUME -eq 0 ]]; then
    git status --porcelain | head -10 >&2
    die 1 "拒绝在脏工作树上发版"
  fi
fi

if [[ "$RELEASE_DIRTY" -gt 0 ]]; then
  run git add "${RELEASE_FILES[@]}"
  run git commit -m "chore(release): ${NEW_VERSION}"
else
  echo "  release 三件套无改动，跳过 commit（resume 模式）"
fi

if [[ $DRY_RUN -eq 0 ]]; then
  run git push -u origin "$RELEASE_BRANCH"
else
  echo "  [dry-run] 跳过 git push"
fi

# ────── Step 5: 开 PR ──────
step "Step 5: 开 PR"

PR_NUMBER=""
PR_STATE=""
if [[ $DRY_RUN -eq 0 ]]; then
  # 检查是否已存在 PR（任意状态：open/merged/closed 都要查到，避免 resume 重开）
  PR_INFO=$(gh pr list --state all --head "${RELEASE_BRANCH}" --json number,state --jq '.[0] // {}')
  PR_NUMBER=$(echo "${PR_INFO}" | jq -r '.number // ""')
  PR_STATE=$(echo "${PR_INFO}" | jq -r '.state // ""')

  if [[ -n "${PR_NUMBER}" && "${PR_STATE}" == "MERGED" ]]; then
    echo "  PR #${PR_NUMBER} 已 merged，跳过开 PR / wait-ci / merge（resume 续跑到 tag 步骤）"
    SKIP_TO_TAG=1
  elif [[ -n "${PR_NUMBER}" ]]; then
    echo "  PR #${PR_NUMBER} 已存在（state=${PR_STATE}），复用"
    SKIP_TO_TAG=0
  else
    SKIP_TO_TAG=0
    PR_BODY=$(cat <<EOF
## Release ${NEW_VERSION}

由 \`scripts/release.sh\` 自动生成。

- 三处版本号已同步至 \`${NEW_VERSION}\`
- \`Cargo.lock\` / \`src-tauri/Cargo.lock\` 已同步刷新
- \`just preflight\` 已通过

合 PR 后脚本会自动：tag \`v${NEW_VERSION}\` → push tag → 监控 \`release.yml\` → draft ready。
EOF
)
    gh pr create \
      --title "chore(release): ${NEW_VERSION}" \
      --body "${PR_BODY}" \
      --base main \
      --head "${RELEASE_BRANCH}" >/dev/null
    # 用 view --json 取 number，比 grep URL 文本更稳
    PR_NUMBER=$(gh pr view "${RELEASE_BRANCH}" --json number --jq '.number' 2>/dev/null || echo "")
    if [[ -z "${PR_NUMBER}" ]]; then
      die 99 "gh pr create 后无法取到 PR number；gh pr list --head ${RELEASE_BRANCH} 看一下"
    fi
  fi
  echo "  PR #${PR_NUMBER}"
else
  echo "  [dry-run] 跳过 gh pr create"
  PR_NUMBER="<dry>"
  SKIP_TO_TAG=0
fi

# ────── Step 6: wait-ci ──────
step "Step 6: wait-ci PR #${PR_NUMBER}"

if [[ ${SKIP_TO_TAG:-0} -eq 1 ]]; then
  echo "  PR 已 merged，跳过"
elif [[ $DRY_RUN -eq 0 ]]; then
  # gh pr checks --watch 会阻塞直到全绿/失败
  if ! gh pr checks "${PR_NUMBER}" --watch --interval 30; then
    echo "" >&2
    echo "  失败 job 日志摘要:" >&2
    gh run list --limit 3 --branch "${RELEASE_BRANCH}" --json databaseId,name,conclusion,status \
      --jq '.[] | select(.conclusion=="failure") | "  - \(.name) (run \(.databaseId))"' >&2 || true
    die 3 "PR CI 红，请 gh run view --log-failed 定位 + 修 + push 后用 --resume ${NEW_VERSION} 续跑"
  fi
else
  echo "  [dry-run] 跳过 wait-ci"
fi

# ────── Step 7: squash merge ──────
step "Step 7: squash merge PR #${PR_NUMBER}"

if [[ ${SKIP_TO_TAG:-0} -eq 1 ]]; then
  echo "  PR 已 merged，跳过"
elif [[ $DRY_RUN -eq 0 ]]; then
  run gh pr merge "${PR_NUMBER}" --squash --delete-branch
else
  echo "  [dry-run] 跳过 gh pr merge"
fi

# ────── Step 8: 切 main + tag ──────
step "Step 8: tag v${NEW_VERSION}"

run git checkout main
run git pull origin main
run git fetch --tags origin

# 检查 tag 是否已存在（本地 + 远端）
TAG_LOCAL=$(git rev-parse -q --verify "refs/tags/v${NEW_VERSION}" 2>/dev/null || echo "")
TAG_REMOTE=$(git ls-remote --tags origin "refs/tags/v${NEW_VERSION}" | awk '{print $1}')
if [[ -n "${TAG_LOCAL}" || -n "${TAG_REMOTE}" ]]; then
  echo "  tag v${NEW_VERSION} 已存在 (local=${TAG_LOCAL:-no}, remote=${TAG_REMOTE:-no})，跳过创建"
  if [[ -z "${TAG_LOCAL}" && -n "${TAG_REMOTE}" ]]; then
    echo "  从远端拉取 tag 到本地..."
    run git fetch origin "refs/tags/v${NEW_VERSION}:refs/tags/v${NEW_VERSION}"
  fi
else
  run git tag "v${NEW_VERSION}"
fi

if [[ -z "${TAG_REMOTE}" ]]; then
  if [[ $DRY_RUN -eq 0 ]]; then
    run git push origin "v${NEW_VERSION}"
  else
    echo "  [dry-run] 跳过 git push tag"
  fi
else
  echo "  远端 tag 已存在，跳过 push"
fi

# ────── Step 9: 监控 release.yml ──────
step "Step 9: 监控 release.yml"

if [[ $DRY_RUN -eq 0 ]]; then
  # 等 workflow 启动（最多 60s）
  RUN_ID=""
  # 精确匹配本次 tag 触发的 run：headBranch == "v$NEW_VERSION" + event=="push"。
  # 只筛 event=="push" 不够 —— main push 也是 push，会拿到无关 run。
  TAG_REF="v${NEW_VERSION}"
  # 上限 5 分钟（GitHub Actions queue 偶尔 > 1min）
  for i in $(seq 1 30); do
    sleep 10
    RUN_ID=$(gh run list --workflow=release.yml --limit 10 \
      --json databaseId,headBranch,event,status \
      --jq "[.[] | select(.event==\"push\" and .headBranch==\"${TAG_REF}\") | .databaseId] | .[0] // \"\"")
    if [[ -n "${RUN_ID}" ]]; then break; fi
    if [[ $((i % 6)) -eq 0 ]]; then
      echo "  等 release.yml 启动 (${i}0s elapsed)..."
    fi
  done

  if [[ -z "${RUN_ID}" ]]; then
    die 4 "release.yml 5min 内未识别到 ${TAG_REF} 触发的 run；gh run list --workflow=release.yml 看一下"
  fi

  echo "  release.yml run: ${RUN_ID} (tag ${TAG_REF})"
  if ! gh run watch "${RUN_ID}" --interval 30 --exit-status; then
    echo "" >&2
    echo "  release.yml 红了，看一眼是否命中已知 fix:" >&2
    echo "    F1: 4 个 draft 各只含一个平台 → workflow 缺 create-release 前置 job（改 .github/workflows/release.yml）" >&2
    echo "    F3: macos-13 runner 不可用 → 升 macos-14；或 apt 包冲突 → 加 apt-get update / purge" >&2
    echo "    F4: minisign 校验失败 → GitHub secret 链 / Cargo.toml plugin / lib.rs 注册任一缺" >&2
    die 4 "gh run view ${RUN_ID} --log-failed 定位 + 修 + 重打 tag"
  fi
else
  echo "  [dry-run] 跳过 gh run watch"
fi

# ────── Step 10: 验 asset 齐全 ──────
step "Step 10: 验 4 平台 asset + minisign 签名链齐全"

if [[ $DRY_RUN -eq 0 ]]; then
  ASSET_NAMES=$(gh release view "v${NEW_VERSION}" --json assets --jq '.assets[].name')
  ASSET_COUNT=$(echo "${ASSET_NAMES}" | wc -l | tr -d ' ')
  echo "  assets: ${ASSET_COUNT} 个"
  echo "${ASSET_NAMES}" | sed 's/^/    - /'

  # 必有项基线（基于 v0.5.5 实际产出 17 个 asset）：
  #   macOS: aarch64.dmg + x64.dmg + aarch64.app.tar.gz + x64.app.tar.gz（updater bundle）
  #   Linux: .deb + .AppImage + .rpm
  #   Windows: .msi + .exe (NSIS)
  #   updater: latest.json
  MISSING=()
  echo "${ASSET_NAMES}" | grep -qE 'aarch64\.dmg$'                || MISSING+=("macos-arm64 dmg")
  echo "${ASSET_NAMES}" | grep -qE 'x64\.dmg$'                    || MISSING+=("macos-x64 dmg")
  echo "${ASSET_NAMES}" | grep -qE 'aarch64\.app\.tar\.gz$'       || MISSING+=("macos-arm64 .app.tar.gz (updater)")
  echo "${ASSET_NAMES}" | grep -qE 'x64\.app\.tar\.gz$'           || MISSING+=("macos-x64 .app.tar.gz (updater)")
  echo "${ASSET_NAMES}" | grep -qE '\.deb$'                       || MISSING+=("linux .deb")
  echo "${ASSET_NAMES}" | grep -qE '\.AppImage$'                  || MISSING+=("linux .AppImage")
  echo "${ASSET_NAMES}" | grep -qE '\.rpm$'                       || MISSING+=("linux .rpm")
  echo "${ASSET_NAMES}" | grep -qE '\.msi$'                       || MISSING+=("windows .msi")
  echo "${ASSET_NAMES}" | grep -qE 'setup\.exe$'                  || MISSING+=("windows .exe (NSIS)")
  echo "${ASSET_NAMES}" | grep -q 'latest.json'                   || MISSING+=("latest.json (updater manifest)")

  # minisign 签名链：每个安装包应有 .sig（除 latest.json）
  INSTALLER_COUNT=$(echo "${ASSET_NAMES}" | grep -cE '\.(dmg|deb|AppImage|rpm|msi|exe|app\.tar\.gz)$' || echo 0)
  SIG_COUNT=$(echo "${ASSET_NAMES}" | grep -cE '\.sig$' || echo 0)
  if [[ "${INSTALLER_COUNT}" != "${SIG_COUNT}" ]]; then
    echo "" >&2
    echo "  minisign 签名数不匹配：${INSTALLER_COUNT} 个安装包 vs ${SIG_COUNT} 个 .sig" >&2
    echo "  F4 检查链：tauri.conf.json::bundle.createUpdaterArtifacts / GitHub secret TAURI_SIGNING_PRIVATE_KEY{,_PASSWORD}" >&2
    MISSING+=("minisign 签名不齐 (${INSTALLER_COUNT} installer / ${SIG_COUNT} sig)")
  fi

  if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo "" >&2
    echo "  缺以下 asset:" >&2
    printf "    - %s\n" "${MISSING[@]}" >&2
    die 5 "asset 不齐，gh release view v${NEW_VERSION} 排查"
  fi
else
  echo "  [dry-run] 跳过 asset 校验"
fi

# ────── 完成 ──────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Draft release v$NEW_VERSION ready"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "下一步（留 agent / 用户决策）："
echo "  1. gh release view v$NEW_VERSION             # 检查 release notes"
echo "  2. 在 4 个平台之一手动验装一次（推荐）"
echo "  3. gh release edit v$NEW_VERSION --draft=false  # publish"
echo ""
exit 0

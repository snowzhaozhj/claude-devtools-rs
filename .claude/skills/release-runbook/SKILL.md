---
name: release-runbook
description: 完整发版流水线（bump → release-check → PR → wait-ci → merge → tag → 监控 release.yml → publish draft）+ v0.1.0–v0.4.2 已踩坑的 known-failures playbook。用户 `/release-runbook X.Y.Z` 或"发版 / release / bump 版本到 X.Y.Z / 发个 patch"时触发。
---

# release-runbook

> 触发：`/release-runbook 0.5.0`；或自然语言"发个 0.5.0 / 我们 release / bump 到 0.5.0"
> 输出：步进式执行 + 每步状态报告
> 修改：三处版本号、Cargo.lock、Cargo.lock (src-tauri)，不动业务代码

## 一句话流程

```
分支 → bump 三处 → just release-check → amend lock → push PR → wait-ci → merge →
main 拉新 → tag vX.Y.Z → push tag → 监控 release.yml → 4 平台 asset 全到位 → publish draft
```

## 已知失败 playbook（按历史出现频次排序）

逐项遇到时直接套用 fix——这都是 v0.1.0–v0.4.2 期间真实踩过的坑。

### F1. release.yml matrix race（v0.4.2 案例）

**症状**：tag push 后产生 4 个 draft release，每个只含一个平台的 asset，无法 publish。

**根因**：4 个 matrix job 各自跑 `tauri-action` 并 `releaseDraft: true` → 各自调 createRelease → race。

**修法**：workflow 必须是 `create-release` 前置 job 产出 `releaseId`，`build` matrix 通过 `with: releaseId: ${{ needs.create-release.outputs.id }}` 复用。检查 `.github/workflows/release.yml` 是否已是这种结构（CLAUDE.md "发布与分支策略"段已记，但 fork / rebase 后可能丢失）。

### F2. Windows MSI version 含字母（CLAUDE.md 已记，仍易踩）

**症状**：Windows runner 报 `pre-release identifier must be numeric-only`。

**修法**：版本号 SHALL 是 `X.Y.Z` 纯数字（含 hotfix `0.3.1`），**不**用 `X.Y.Z-rc.1` / `X.Y.Z-beta`。演练应用内更新走真实 hotfix release，不走 rc。

### F3. macos runner 不可用 / apt 冲突

**症状（macos）**：`macos-13` runner 被 GitHub 下线，job fail "no runner available"。
**修法**：升级到 `macos-14`（arm64）；若历史 build 依赖 x86_64 ldd 等，改 `macos-13-large` 或 `macos-latest-large`。

**症状（linux apt）**：`E: Unable to locate package <X>` 或 `dpkg: error processing archive`。
**修法**：在 `apt-get install` 前加 `apt-get update`；冲突包加 `apt-get -y purge <pkg>` 清掉再装。

### F4. minisign 签名密钥配置（CLAUDE.md 已记）

**症状**：release bundle 不签名 / 应用内升级签名校验失败。

**前置检查链**（任一缺失即重新配置）：
1. `tauri.conf.json::bundle.createUpdaterArtifacts: true`
2. `tauri.conf.json::plugins.updater.{endpoints,pubkey}` 已填
3. `capabilities/default.json::permissions` 含 `updater:default` + `process:default`
4. `Cargo.toml` 含 `tauri-plugin-updater` + `tauri-plugin-process`
5. `lib.rs` 注册两个 plugin
6. `release.yml` env 注入 `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（GitHub Actions secret）

**最大坑**：私钥不可换。pubkey 已 commit 入库，老用户客户端只信旧公钥。

### F5. `Cargo.lock` 与 manifest 不一致

**症状**：release PR 上 CI 报 `lock file out of date`。

**根因**：`just release-check` 跑会改 `Cargo.lock` + `src-tauri/Cargo.lock`，但 release commit 没 amend。

**修法**：bump 版本号后**先**跑 `just release-check`，再 amend lock 进同一 commit；不要先 commit bump 再跑 check。

### F6. macOS Gatekeeper（用户安装报"无法验证"）

**症状**：用户首次启动 .app 报 "无法打开，Apple 无法验证"。

**根因**：non-notarized；本仓暂不走 Apple 公证（成本 / 流程）。

**修法**（用户侧）：右键 → 打开 → 跳过 Gatekeeper；或终端 `xattr -cr /Applications/<app>.app`。release notes 模板 SHALL 包含这两个步骤。

### F7. Linux `.deb` 不支持应用内升级

**症状**：`update.downloadAndInstall()` 抛错。

**根因**：Tauri 限制——deb 无 in-place 升级机制。

**修法**：UI 层捕获该错误后弹"请到 GitHub 下载新版本"对话框（已实现）；release notes 提示 Linux 用户手动下载。

## 工作步骤

### Step 0：preflight

```bash
git branch --show-current  # 必须不是 main
git status --short          # 必须干净
```

不满足就先跑 `/preflight` 或让用户切分支。

### Step 1：bump 三处版本号

调用 `/bump-version <version>` skill（已存在），或手动改：
- `Cargo.toml` workspace `[workspace.package].version`
- `src-tauri/Cargo.toml` `[package].version`
- `src-tauri/tauri.conf.json` `version`

### Step 2：`just release-check`

会改 `Cargo.lock` + `src-tauri/Cargo.lock`。看是否有 diff。

### Step 3：commit + push + 开 PR

commit message 模板：`chore(release): X.Y.Z`。

```bash
git add Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "chore(release): X.Y.Z"
git push -u origin chore/release-X.Y.Z
gh pr create --title "chore(release): X.Y.Z" --body "..."
```

### Step 4：等 CI 全绿（`/wait-ci` skill）

任一 job 红：先看 F1–F7 是否命中已知 fix；命中则改 `.github/workflows/release.yml` 或 manifest 后 push fix commit；不命中则报告用户介入。

### Step 5：merge + tag

```bash
gh pr merge <pr> --squash --delete-branch
git checkout main && git pull
git tag vX.Y.Z
git push origin vX.Y.Z
```

### Step 6：监控 release.yml

```bash
gh run list --workflow=release.yml --limit 3
gh run watch <run-id>
```

任一 platform job 红：套 F1-F4 fix。

### Step 7：4 平台 asset 全到位 → publish

```bash
gh release view vX.Y.Z --json assets -q '.assets | length'
```

期望 ≥ 4（macOS arm64 + macOS x86_64 + Linux deb + Linux AppImage + Windows MSI 等）。全到位后 `gh release edit vX.Y.Z --draft=false` publish。

## 不要做

- 不要在 main 上 bump（一定走 PR）
- 不要在 release commit 之后再跑 `just release-check`——lock 一定会和 manifest 脱节
- 不要尝试 rotate minisign 私钥（详见 F4 最大坑）
- 不要给 tag 名加 rc / beta（F2）

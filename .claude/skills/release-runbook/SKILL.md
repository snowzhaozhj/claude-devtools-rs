---
name: release-runbook
description: 完整发版流水线（preflight → bump → release-check → PR → wait-ci → merge → tag → 监控 release.yml → publish draft）+ v0.1.0–v0.5.3 已踩坑的 known-failures playbook（matrix race / Windows MSI 字母版本号 / minisign 链 / lock 不同步 / Gatekeeper / Linux .deb 升级限制）。**用户说 `/release-runbook X.Y.Z` 或"发版 / release / 发个 patch / bump 到 X.Y.Z / 发 vX"时都用这个 skill**，不要自己散步骤跑——历史上每个 step 都有踩过的坑。
---

# release-runbook

> 触发：`/release-runbook 0.5.4`；或自然语言"发个 0.5.4 / 我们 release / bump 到 0.5.4 / 发个 patch / 出个新版本"
> 输出：步进式执行 + 每步状态报告
> 修改：三处版本号 + Cargo.lock + src-tauri/Cargo.lock；不动业务代码

## 一句话流程

```
preflight → 分支 → just release-bump X.Y.Z (bump 三处 + release-check + commit) →
push PR → /wait-ci → codex 二审（按豁免规则可跳）→ merge →
main 拉新 → tag vX.Y.Z → push tag → 监控 release.yml
（workflow 末尾 publish job 自动 verify 17 个 asset + un-draft，无需人工 publish）
```

**自动化点**（v0.5.5 之后）：
- `just release-bump X.Y.Z` 把"sed 三处 + release-check + git add + commit"做成一步（详 `scripts/release-bump.sh`）
- `release.yml` 末尾 `publish` job 自动校验 17 个必需 asset + `gh release edit --draft=false`，缺一即 fail 保留 draft 待人工介入

Agent / 人工只在 **CI 红 / workflow build job 失败 / verify-asset 缺件** 三种异常时介入。

## 何时用 release-engineer subagent vs 本 skill

- **本 skill（手把手）**：日常 patch（0.x.y bump、单 hotfix）；要看清每一步、需要在某步停下沟通。
- **`release-engineer` subagent**（`Agent({ subagent_type: "release-engineer", ... })`）：大版本发版懒得手把手，subagent 在隔离 context 里自治推进，遇 known-failure 自动套 playbook fix，只在 playbook 不命中时回主对话升级。

两者共享同一 playbook（F1-F7）；subagent 是把本 skill 跑到底的自动化版本。

## 何时用 bg dispatch

- **不用**：发版本身是主线流程，需要随时看 CI 红绿决策——不要用 `claude --bg`。
- 例外：如果发版触发的 CI 期间你想并行干**另一件不相关的事**，那件事可以走 bg dispatch（参考 `.claude/rules/bg-task-dispatch.md`）。发版自身留主线。

## 已知失败 playbook（按历史出现频次排序）

逐项遇到时直接套用 fix——这都是 v0.1.0–v0.5.3 期间真实踩过的坑。

### F1. release.yml matrix race（v0.4.2 案例）

**症状**：tag push 后产生 4 个 draft release，每个只含一个平台的 asset，无法 publish。

**根因**：4 个 matrix job 各自跑 `tauri-action` 并 `releaseDraft: true` → 各自调 createRelease → race。

**修法**：workflow 必须是 `create-release` 前置 job 产出 `releaseId`，`build` matrix 通过 `with: releaseId: ${{ needs.create-release.outputs.id }}` 复用。`tauri-action` 在给定 `releaseId` 时只上传 + merge `latest.json`，不再调 createRelease。检查 `.github/workflows/release.yml` 是否已是这种结构（CLAUDE.md "发布与分支策略"段已记，但 fork / rebase 后可能丢失）。

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

**修法**：bump 版本号后**先**跑 `just release-check`，再把改动的 lock 一起 amend / 重新 commit；不要先 commit bump 再跑 check。CLAUDE.md "发布与分支策略"段已固化。

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

先调 `preflight` skill。要求：
- origin/main 已 fetch + 同步
- 当前分支非 main / master——切到 `chore/release-X.Y.Z`
- 工作树干净

不满足就先解决。

### Step 1：本地 bump + commit（一行）

```bash
just release-bump X.Y.Z
```

`scripts/release-bump.sh` 内部：
1. 校验版本号格式（X.Y.Z 纯数字，拒 -rc/-beta，详 F2）
2. 校验当前分支非 main / 工作树干净
3. sed bump 三处版本号（`Cargo.toml` workspace + `src-tauri/Cargo.toml` + `src-tauri/tauri.conf.json`）
4. 跑 `just release-check`（同步刷新两份 `Cargo.lock`，避免 F5）
5. `git add` 固定 5 文件 + `git commit -m "chore(release): X.Y.Z"`

不 push、不 open PR——这两步保留 Agent / 人工拍板。

### Step 2：push + 开 PR

```bash
git push -u origin chore/release-X.Y.Z
gh pr create --title "chore(release): X.Y.Z" --body "..."
```

### Step 3：等 CI 全绿（调用 `wait-ci` skill）

调用 `wait-ci` skill polling。任一 job 红：先看 F1–F7 是否命中已知 fix；命中则改 workflow 或 manifest 后 push fix commit；不命中则报告用户介入。

### Step 4：codex 二审

按 `.claude/rules/codex-usage.md` 第 1 节判断——release commit 通常属于"显式豁免（bump version）"范畴，可跳过 codex 二审，但 PR 描述里写一句"未跑 codex（理由：纯 version bump）"留痕。如果这个 release 顺带改了 release.yml / Cargo manifest 不止版本号字段，那不算"纯 bump"，仍要跑 codex。

### Step 5：merge + tag

```bash
gh pr merge <pr> --squash --delete-branch
git checkout main && git pull
git tag vX.Y.Z
git push origin vX.Y.Z
```

### Step 6：监控 release.yml（自动 verify + publish）

```bash
gh run list --workflow=release.yml --limit 3
gh run watch <run-id> --exit-status --interval 30
```

调用方式：Bash 工具 **`run_in_background: true`** + `timeout: 3600000`（1 h 上限）。release.yml 跨平台矩阵 + bundle 通常 20-30 min，远超 Bash 工具同步 10 min 上限；run_in_background 让 harness 在 `gh run watch` 退出时自动 task-notification 触发，避免 Bash timeout 误判失败。退出码：0 全绿、非 0 有 job 失败。**不要**主动 tail 输出 / ScheduleWakeup poll——bg 模式下 harness 已经在等 exit 信号，主动看是浪费 token。

workflow 结构：`create-release` → `build (matrix×4)` → `publish`（verify 17 个必需 asset + un-draft）。conclusion=success 即发版完成。

- 任一 build job 红：套 F1–F4 fix（matrix race / Windows MSI 字母版本号 / runner / minisign）
- `publish` job 红：通常是某平台 asset 缺失——`gh release view vX.Y.Z --json assets -q '.assets[].name'` 对比 workflow `REQUIRED` 列表定位；常见是 build 跑了但 tauri-action 没上传成功，re-run failed jobs 即可
- 全绿 → release 已自动 publish，不需要人工 `gh release edit --draft=false`

仅当 workflow 内 publish job 缺失（fork / 老版本）时才回退到手工：
```bash
gh release edit vX.Y.Z --draft=false
```

## 不要做

- 不要在 main 上 bump（一定走 PR）
- 不要在 release commit 之后再跑 `just release-check`——lock 一定会和 manifest 脱节（F5）
- 不要尝试 rotate minisign 私钥（F4 最大坑）
- 不要给 tag 名加 rc / beta（F2）
- 不要在发版本身上用 `claude --bg`——发版需要随时看 CI 状态决策；如果你想并行干别的事，那件事走 bg，发版留主线

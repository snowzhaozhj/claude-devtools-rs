---
name: bump-version
description: 同步 bump workspace 版本号三处（Cargo.toml / src-tauri/Cargo.toml / src-tauri/tauri.conf.json）→ 跑 `just release-check`（版本一致 + preflight + lock 同步）。**只在**用户显式 `/bump-version <new>` 或自然语言"bump 版本 / 升版本号 / bump 到 X.Y.Z / 发 vX"时触发——模型不能自主调用，因为这是发版动作必须用户授权。完整发版流程见 `release-runbook` skill。
disable-model-invocation: true
---

# bump-version

**触发**：用户显式 `/bump-version <new-version>` 或自然语言"bump 到 0.1.2 / 升版本 / 准备发 vX.Y.Z"。

**不触发**：任何没有明确版本号的场景。模型自主调用已禁用（`disable-model-invocation: true`）—— bump 版本是有副作用的发版动作，必须用户授权。

## 输入

- 新版本号（SemVer，形如 `0.1.1` / `1.2.3`）

约束：
- 必须是纯数字 `X.Y.Z`——**禁止** `0.3.0-rc.1` / `0.3.0-beta` 等含字母后缀（CLAUDE.md "发布与分支策略"段：Windows MSI bundler 不接受 pre-release 含字母）
- 若用户没给版本号：用 AskUserQuestion 问一次，不要自己猜

## 三处同步点（必须一致，否则 `just release-check` 拒）

1. `Cargo.toml` workspace 根：`[workspace.package]` 下 `version = "X.Y.Z"`
2. `src-tauri/Cargo.toml` 独立 manifest：`[package]` 下 `version = "X.Y.Z"`
3. `src-tauri/tauri.conf.json` JSON：`"version": "X.Y.Z"`

## 工作步骤

### 1. 读取当前版本

```bash
grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
grep -E '^version\s*=' src-tauri/Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
grep -E '"version":' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/'
```

三处应当相等（`just release-check` 的前置不变量）。若不一致：**停下来**报告当前状态、让用户决策以谁为基准，再统一——不要强行 bump，可能覆盖别人未 push 的改动。

### 2. 用 Edit 工具改三处

**禁止用 `sed -i`**（BSD/macOS 需要空串参数，易出错；Edit 工具有 stale-read 保护）。

- `Cargo.toml` 的 `[workspace.package]` 段里 `version = "<old>"` → `version = "<new>"`
- `src-tauri/Cargo.toml` 的顶层 `[package]` 段里 `version = "<old>"` → `version = "<new>"`
- `src-tauri/tauri.conf.json` 的 `"version": "<old>",` → `"version": "<new>",`

### 3. 跑 `just release-check`

```bash
just release-check
```

该命令检查三处版本一致 + 工作树干净 + 跑 `just preflight`（fmt + lint + test + spec-validate）。

**预期**：工作树检查会**失败**——因为刚改了三处。这是设计：`just release-check` 是"commit 前最后一步"，假设此时工作树已经干净。本 skill 的预期是"先 bump 再 commit"，所以**跳过**工作树干净检查，直接跑 preflight 等效命令验证：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
openspec validate --all --strict
```

四项全绿即可。

**注意**：`just release-check` / `cargo check` 会把 `Cargo.lock` + `src-tauri/Cargo.lock` 改掉（因为 workspace 版本变了）——这两个 lock 文件**也属于本次 bump 的产物**，commit 时必须一起 add（见 release-runbook F5）。

### 4. 报告给用户

输出：
- 三处版本号的新值（grep 再确认一次）
- preflight 四项结果
- lock 文件是否被 cargo 重写（`git status` 看 `Cargo.lock` / `src-tauri/Cargo.lock` 是否进 staged 区）
- 下一步手动动作（不自动跑）：
  - `git add Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json`
  - `git commit -m "chore(release): vX.Y.Z"`
  - push + 开 PR → merge → 在 main 上 `git tag vX.Y.Z && git push origin vX.Y.Z`

**不自动 commit / 不自动打 tag**——发版动作必须用户手动收尾。

## 硬性约束

- 不写注释、不动业务代码——只改三处版本字符串 + 触发 cargo 重生 lock
- 若用户传入的 `<new>` 不是合法 SemVer 纯数字（如 `v0.1.1`、`0.1` 缺 patch、`0.3.0-rc.1` 含字母）：拒绝并让用户重输
- 若三处当前版本不一致：**不要**强行 bump，先让用户确认基准
- 完整发版流水线（push → wait-ci → merge → tag → 监控 release.yml → publish）走 `release-runbook` skill，本 skill 只管"改三处 + verify"

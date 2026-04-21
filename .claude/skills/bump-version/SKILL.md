---
name: bump-version
description: 同步 bump workspace 版本号三处（Cargo.toml / src-tauri/Cargo.toml / src-tauri/tauri.conf.json）；然后跑 just release-check（版本一致 + 工作树干净 + preflight）。用户显式 `/bump-version <new>` 或"bump 版本 / 升版本号 / 发 vX"时触发。不在 CI / 自动流程里调用。
disable-model-invocation: true
---

# bump-version

**触发**：用户显式 `/bump-version <new-version>` 或自然语言"bump 到 0.1.2 / 升版本 / 准备发 vX.Y.Z"。

**不触发**：任何没有明确版本号的场景。模型自主调用已禁用（`disable-model-invocation: true`）—— bump 版本是有副作用的发版动作，必须用户授权。

## 输入

- 新版本号（SemVer，形如 `0.1.1` / `1.2.3-rc.1`）

若用户没给版本号：**用 AskUserQuestion 问一次，不要自己猜**。

## 三处同步点（必须一致，否则 `just release-check` 拒）

1. `Cargo.toml` workspace 根：`[workspace.package]` 下 `version = "X.Y.Z"`
2. `src-tauri/Cargo.toml` 独立 manifest：`[package]` 下 `version = "X.Y.Z"`
3. `src-tauri/tauri.conf.json` JSON：`"version": "X.Y.Z"`

## 工作步骤

按 .claude/rules/opsx-apply-cadence.md 的节拍推进，每步不中断。

### 1. 读取当前版本（用 grep / head 直接拿字符串）

```bash
grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
grep -E '^version\s*=' src-tauri/Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
grep -E '"version":' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/'
```

三处应当相等（`just release-check` 的前置不变量）。若不一致，先报告当前状态、让用户决策以谁为基准，再统一。

### 2. 用 Edit 工具改三处

**禁止用 sed -i**（`sed -i` 在 BSD/macOS 需要空串参数，易出错；Edit 工具有 stale-read 保护）。

- `Cargo.toml` 的 `[workspace.package]` 段里 `version = "<old>"` → `version = "<new>"`
- `src-tauri/Cargo.toml` 的顶层 `[package]` 段里 `version = "<old>"` → `version = "<new>"`
- `src-tauri/tauri.conf.json` 的 `"version": "<old>",` → `"version": "<new>",`

三处用 Edit 做精确替换。

### 3. 跑 `just release-check`

```bash
just release-check
```

该命令：
- 核对三处版本一致（通过前面 Edit 就会一致）
- 检查工作树状态：若有未提交改动会先打印提醒；按本 skill 流程此时**预期工作树有改动**（刚改的三处），release-check 会 fail 此检查 —— 这不是错误，是 release-check 的语义是 "commit 前最后一步"
- 跑 `just preflight`（fmt + lint + test + spec-validate）

**调整**：本 skill 的预期是"先 bump 再 commit"，所以实际顺序是：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
openspec validate --all --strict
```

手动跑 preflight 等效子命令（跳过 release-check 的"工作树干净"检查），verify 四项全绿。

### 4. 报告给用户

输出：
- 三处版本号的新值（grep 再确认一次）
- preflight 四项结果
- 下一步手动动作：
  - `git add Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json`
  - `git commit -m "chore(release): vX.Y.Z"`
  - push + 开 PR → merge → 打 tag `git tag vX.Y.Z && git push origin vX.Y.Z`

**不自动 commit / 不自动打 tag**。这是发版动作，必须用户手动收尾。

## 边界 / 约束

- 如果 Cargo.lock 因版本号变化被触发重生，顺手 `cargo check --workspace` 让它更新即可；不需要手动改 Cargo.lock
- 若用户传入的 `<new>` 不是合法 SemVer（如 `v0.1.1`、`0.1` 缺 patch 位），拒绝并让用户重输
- 若三处当前版本**不一致**，**不要**强行 bump —— 先让用户确认基准，避免覆盖别人未 push 的改动

---
name: insta-review
description: 审查并接受 insta 快照变更。以 `INSTA_UPDATE=no` 跑测试产出 `.snap.new`，逐一 diff 让用户确认，通过后以 `INSTA_UPDATE=always` 接受并 `git add tests/snapshots/`。当用户说 "review snapshots"、"更新快照"、"接受 insta" 或 `/insta-review` 时触发。
---

# insta-review

claude-devtools-rs 用 `insta` crate 做快照测试（`cdt-analyze/tests/chunks.rs` 等）。本 skill 封装 CLAUDE.md 里提到的"没装 `cargo-insta` 就用 `INSTA_UPDATE=always`"流程，避免两步：`INSTA_UPDATE=no` → diff → `INSTA_UPDATE=always` → `git add`。

## 输入

可选参数：目标 crate 名（kebab-case）。省略时默认 `cdt-analyze`（当前唯一使用 insta 的 crate；未来新增需更新此默认值）。

## 工作步骤

### 1. 确认 crate

如用户给了参数则用它；否则默认 `cdt-analyze`。用 `Grep` 确认该 crate 有 `insta::` 调用 + `tests/snapshots/` 目录存在，若缺任一则报 error 退出。

### 2. 检测"接受模式"可用性

检查 `cargo insta --version`：

- 成功 → 走 **路径 A**（`cargo insta review/accept`）
- 失败 → 走 **路径 B**（`INSTA_UPDATE` 环境变量）

### 3. 路径 A：cargo-insta 已装

1. `cargo test -p <crate>`（默认 `INSTA_UPDATE=no`，失败时产出 `.snap.new`）
2. `cargo insta pending-snapshots -p <crate>` 列待接受项
3. **逐一展示 diff 给用户**：对每个 `.snap.new`，读原 `.snap` + 新 `.snap.new`，输出 unified diff（用 `Read` + 内存 diff；或 `git diff --no-index`）
4. 用户答"接受"后跑 `cargo insta accept -p <crate>`
5. 用户答"全部拒绝"后跑 `cargo insta reject -p <crate>`
6. 若用户要求部分接受，列出 `cargo insta review` 的交互用法，请用户在终端自己跑——skill 不代跑交互式命令

### 4. 路径 B：cargo-insta 未装

1. `INSTA_UPDATE=no cargo test -p <crate>` —— 此命令在有 snapshot 变更时会 **失败**（退出码非 0），但会在 `tests/snapshots/` 下生成 `*.snap.new`
2. `git status -- crates/<crate>/tests/snapshots/` 列所有 `.snap.new`
3. 对每个 `.snap.new`：
   - 读对应 `.snap`（旧）与 `.snap.new`（新）
   - 输出 unified diff 到聊天
4. 用户确认后：`INSTA_UPDATE=always cargo test -p <crate>` —— 覆盖 `.snap` 并删除 `.snap.new`
5. 用户拒绝时：`find crates/<crate>/tests/snapshots/ -name "*.snap.new" -delete`

### 5. 暂存变更

接受后：`git add crates/<crate>/tests/snapshots/`。**不** commit——commit 留给用户或 `commit-commands:commit` skill。

## 输出格式

每步执行前简短描述在做什么（1 行），每个 diff 用 ```diff 代码块包起来。最终一行总结：`接受了 N 个快照变更，已 git add；未 commit`。

## 硬性约束

- 不自动 `git commit`。
- 不修改 source（`src/**/*.rs`）或测试（`tests/**/*.rs`）——只动 `tests/snapshots/*.snap`。
- 若 `INSTA_UPDATE=no` 跑出的失败中混杂非快照相关的真实测试失败，**停止**接受流程，把完整失败输出回传给用户排查。
- 若 crate 没有 `.snap.new`（说明没有快照变更），直接退出并告知"快照已是最新"。

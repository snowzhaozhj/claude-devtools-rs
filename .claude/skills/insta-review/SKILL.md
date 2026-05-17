---
name: insta-review
description: 审查并接受 insta 快照变更。默认走 `INSTA_UPDATE=no` 跑测试产 `.snap.new` → 逐一 diff 让用户确认 → 通过后 `INSTA_UPDATE=always` 覆盖 + `git add tests/snapshots/`。**只要**用户说"review snapshots / 更新快照 / 接受 insta / snapshot 测试挂了 / insta 失败"或显式 `/insta-review`，**都用这个 skill**——不要自己手跑 `INSTA_UPDATE=always` 盲接受，那会把回归当 OK。
---

# insta-review

claude-devtools-rs 用 `insta` crate 做快照测试（`cdt-analyze/tests/chunks.rs` 等）。本 skill 把"`INSTA_UPDATE=no` → 看 diff → `INSTA_UPDATE=always` → `git add`"两步流程标准化，并提供 diff 展示，避免盲接受回归。

## 输入

可选参数：目标 crate 名（kebab-case）。省略时默认 `cdt-analyze`（当前唯一使用 insta 的 crate；如未来新增需更新此默认值——用 `grep -rn "insta::" crates/*/tests` 探测）。

## 工作步骤

### 1. 确认 crate

如用户给了参数则用它；否则默认 `cdt-analyze`。用 `Grep` 确认该 crate 有 `insta::` 调用 + `tests/snapshots/` 目录存在；若缺任一则报 error 退出。

### 2. 跑测试产快照变更

```bash
INSTA_UPDATE=no cargo test -p <crate>
```

`INSTA_UPDATE=no` 是默认行为，但写出来更明确——有 snapshot 变更时此命令会**失败**（退出码非 0），同时在 `crates/<crate>/tests/snapshots/` 下生成 `*.snap.new`。失败本身是预期，不要把它当作"测试坏了"。

混杂真实测试失败（非快照相关）时：**停止**接受流程，把完整失败输出回传给用户排查。区分方法：失败 message 里有 `snapshot ... was not as expected` 才是快照变更；其他失败（panic / assert）走另一条排查路径。

### 3. 列出待接受项

```bash
git status -- crates/<crate>/tests/snapshots/
```

或：

```bash
find crates/<crate>/tests/snapshots -name "*.snap.new"
```

无 `.snap.new` → 报告"快照已是最新"并退出。

### 4. 逐一展示 diff

对每个 `<name>.snap.new`：

- 读对应 `<name>.snap`（旧）与 `<name>.snap.new`（新）
- 输出 unified diff 到聊天，用 ```` ```diff ```` 代码块包起来
- 每个 diff 前注明对应的测试名 + 文件路径（行号区间）

### 5. 等用户决策

用户说"接受 / 全部接受 / OK"：

```bash
INSTA_UPDATE=always cargo test -p <crate>
git add crates/<crate>/tests/snapshots/
```

`INSTA_UPDATE=always` 会覆盖 `.snap` 并删除 `.snap.new`。

用户说"拒绝 / 全部拒绝 / 回滚"：

```bash
find crates/<crate>/tests/snapshots/ -name "*.snap.new" -delete
```

用户说"部分接受"：本 skill 不代跑交互——告知用户安装 `cargo-insta`（`cargo install cargo-insta`）后用 `cargo insta review -p <crate>` 在终端自己选。本 skill 是"全接 / 全弃"二选一。

### 6. 暂存 + 报告

接受后只 `git add`，**不** commit——commit 留给用户或 `commit-commands:commit` skill。

最终一行总结：`接受了 N 个快照变更，已 git add；未 commit`。

## 硬性约束

- 不自动 `git commit`
- 不修改 source（`src/**/*.rs`）或测试代码（`tests/**/*.rs` 里的非 snapshot 文件）——只动 `tests/snapshots/*.snap`
- 若 `INSTA_UPDATE=no` 跑出的失败中混杂非快照相关的真实测试失败：**停止**接受流程，回传完整 stderr
- 若 crate 没有 `.snap.new`：直接退出并告知"快照已是最新"
- 不预设 `cargo-insta` 已装——默认走 `INSTA_UPDATE` 环境变量路径。`cargo-insta` 仅在用户主动要求"部分接受"时才提及

## 输出格式

每步执行前简短描述（1 行），每个 diff 用 ```` ```diff ```` 代码块包起来。最终一行总结：`接受了 N 个快照变更，已 git add；未 commit`。

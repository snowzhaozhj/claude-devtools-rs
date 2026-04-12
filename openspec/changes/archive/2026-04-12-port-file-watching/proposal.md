## Why

`cdt-watch` crate 目前是空壳——数据层 13 项能力中 file-watching 尚未启动。作为剩余 8 个 port 的第一项，它是 session-search、configuration-management 等后续 capability 的事件源前置依赖，应最先落地。

## What Changes

- 在 `cdt-watch` crate 内实现 `FileWatcher`：
  - 递归监听 `~/.claude/projects/` 下的 `.jsonl` 文件（创建 / 修改 / 删除），经 100ms 去抖后向所有订阅者广播 `FileChangeEvent`
  - 监听 `~/.claude/todos/` 下的 `.json` 文件，广播 `TodoChangeEvent`
  - 使用 `tokio::sync::broadcast` channel 实现多订阅者分发（无重复）
  - 瞬时文件系统错误（权限拒绝、临时锁定）记录 warning 后继续运行，不中止 watcher
- 为 `cdt-core` 添加 `FileChangeEvent` / `TodoChangeEvent` 共享类型
- 补齐 spec 所有 5 个 Scenario 对应的单元测试与集成测试

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

（无——`openspec/specs/file-watching/spec.md` 与 TS 实现完全匹配，`followups.md` 标记 ✅ 完全匹配，无 impl-bug 需要修正，Rust port 直接按现有 spec 实现即可）

## Impact

- **新增实现**：`crates/cdt-watch/src/`（`lib.rs`、`watcher.rs`、`event.rs`、`debounce.rs`）
- **新增类型**：`cdt-core` 中增加 `FileChangeEvent`、`TodoChangeEvent` 结构体
- **依赖**：`notify`（跨平台 fs 事件）、`tokio`（异步 runtime）——`cdt-watch` 已是 leaf crate，允许引入
- **无破坏性变更**：其余 crate 不受影响

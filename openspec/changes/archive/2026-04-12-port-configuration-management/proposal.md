## Why

`configuration-management` 是 13 个 capability 中第 8 个待迁移项，位于 file-watching / session-search 之后。它管理应用持久化配置（triggers、UI 偏好、pinned sessions、HTTP 端口、SSH hosts）、CLAUDE.md 多 scope 读取、以及 `@mention` 路径沙盒校验。下游 `notification-triggers` 和 `context-tracking` 的 `initial_claude_md_injections` 均依赖本 capability 提供的数据。

TS 实现存在一个已知 impl-bug（`followups.md`）：`loadConfig()` 在 config 文件损坏时只 log + 加载默认值，不备份损坏文件。Rust 端口 MUST 按 spec 实现备份行为。

## What Changes

- 在 `cdt-config` crate 实现 `ConfigManager`：加载/保存/合并 `~/.claude/claude-devtools-config.json`，损坏文件自动备份后加载默认值
- 实现分 section 配置更新 + 字段校验（port 范围、regex ReDoS 防护）
- 实现 `TriggerManager`：trigger CRUD + builtin merge + 校验（数据类型 + 持久化，eval 逻辑留给 `port-notification-triggers`）
- 实现 `ClaudeMdReader`：8 scope 读取（enterprise / user / project / project-alt / project-rules / project-local / user-rules / auto-memory）
- 实现 `@mention` 路径解析 + 沙盒校验（敏感文件 pattern 黑名单 + 允许目录白名单 + symlink escape 防护）
- Session pin/unpin、hide/unhide 管理
- **修复 impl-bug**：损坏 config 文件备份（TS 未实现 → Rust 按 spec 实现）

## Capabilities

### New Capabilities

（无新 capability）

### Modified Capabilities

- `configuration-management`：Scenario "Corrupted config file" 行为细化 —— 备份文件命名约定（`.bak.<timestamp>`）、CLAUDE.md 读取 scope 从 spec 的 3 个扩展到 TS 实际实现的 8 个（MODIFIED delta）

## Impact

- **代码**：`crates/cdt-config/src/` —— 从 stub 扩展为完整实现
- **依赖**：新增 `serde`、`serde_json`、`tokio`（async fs）、`regex`、`dirs`（home dir）、`tracing`
- **下游接入**：`cdt-analyze::context` 的 `initial_claude_md_injections` 可接入 `ClaudeMdReader` 输出；`port-notification-triggers` 复用 trigger 数据类型
- **Workspace Cargo.toml**：为 `cdt-config` 添加 workspace 依赖声明

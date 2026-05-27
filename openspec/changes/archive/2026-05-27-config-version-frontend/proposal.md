# Proposal: config-version-frontend

## Problem

后端 `ConfigManager` 已实现 `version: u64` 乐观并发控制（optimistic concurrency），但前端完全不消费该字段：

1. `get_config` IPC/HTTP 返回的 JSON **不含** version
2. 前端 `updateConfig()` **不传** `_version`
3. 多 tab / 多窗口并发改 Settings 时冲突静默丢失

## Solution

闭环 last-write-wins 防御：

1. **后端**：`get_config` 返回体注入 `_version: u64` 顶层字段
2. **前端 store**：引入 `configVersion` 响应式状态，`getConfig()` 后同步
3. **前端发送**：`updateConfig()` 自动附带 `_version`
4. **冲突处理**：后端 mismatch 返回 error → 前端 toast 提示 + 重新 `getConfig()` 同步

## Scope

- Capability: `configuration-management`
- Crates: `cdt-api`（IPC layer 注入 version）、`cdt-config`（已有，无需改）
- UI: `api.ts`、`SettingsView.svelte`、新增 `configStore.svelte.ts`
- 向后兼容：不传 `_version`（None）时跳过检查（已有逻辑）

## Non-goals

- 不加 `config_changed` push 事件（留后续 PR）
- 不改 `ConfigManager` 内部逻辑（已完备）
- 不做自动 merge / 3-way diff

## Why

打开 claude-devtools-rs 这类有几十个会话的项目时，Sidebar 出现明显加载等待（首屏 spinner 持续数秒）。瓶颈已定位：`list_sessions` 对当前页（默认 50）每个会话**顺序全文件扫描** JSONL（单文件最大 4.6 MB），只为算 `title` / `messageCount` / `isOngoing` 三个元数据字段。渲染层也已逼近瓶颈（50 条 DOM 项加上 pin/ongoing 等装饰），未来会话数上涨会进一步恶化。

## What Changes

- **BREAKING**：`list_sessions` 的 `SessionSummary` 元数据契约从"同步返回完整值"改为"骨架立即返回 + 元数据异步推送"：`title` / `messageCount` / `isOngoing` 在初次返回时 SHALL 为 `null` / `0` / `false` 占位，之后通过新增事件 `session-metadata-update` 逐条 patch。
- 后端新增并发元数据扫描任务：用 `tokio::task::JoinSet` + 固定并发度（8）扫当前页所有 session 文件，每扫完一个立即 `broadcast` 发出，Tauri 层桥接 emit 前端事件。
- 前端 Sidebar 订阅 `session-metadata-update`，按 `sessionId` 定位并 in-place 更新 `sessions[i]` 的三个字段；保持 `{#each}` 稳定 key、`silent=true`、不经"加载中..."中间态。
- 前端 Sidebar 会话列表引入虚拟滚动（windowing），仅渲染视口内会话项 + 一定 overscan；保留日期分组头 / PINNED 分区 / OngoingIndicator / pin 图标的既有布局。
- 首屏"加载中..."触发条件收紧：只在骨架数据本身未到达时显示；元数据 pending 期间 SHALL 显示完整列表（title fallback 到 sessionId 前缀，messageCount 显示为 `C`，ongoing 圆点不显示）。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `sidebar-navigation`：新增虚拟滚动承载、元数据增量渲染、骨架加载状态三条 requirement，收紧"加载中..."触发条件。
- `ipc-data-api`：修改 `list_sessions` 契约（元数据占位 + 事件驱动），新增 `session-metadata-update` push event 要求。

## Impact

- **代码**
  - `crates/cdt-api/src/ipc/local.rs`：`list_sessions` 改骨架返回，新增元数据扫描后台任务；新增 `subscribe_session_metadata()` broadcast。
  - `crates/cdt-api/src/ipc/session_metadata.rs`：函数签名/返回值不变，继续复用；调用方从同步 for 循环改为 `JoinSet` 并发池。
  - `crates/cdt-api/src/ipc/traits.rs`：`DataApi` trait 不改；metadata subscribe 作为 `LocalDataApi` 的非 trait 方法（与 trigger CRUD 同模式）。
  - `src-tauri/src/lib.rs`：新增 bridge task 订阅 `subscribe_session_metadata()` → `emit("session-metadata-update", ...)`。
  - `ui/src/lib/api.ts`：`SessionSummary.title` / `messageCount` / `isOngoing` 保持类型签名但语义变为"可能为占位"；新增 `SessionMetadataUpdate` 类型。
  - `ui/src/components/Sidebar.svelte`：引入虚拟滚动（自写或轻依赖），订阅 `session-metadata-update` 事件，in-place patch。
- **依赖**：可能新增 `@tanstack/svelte-virtual` 或自写轻量 virtual list。Rust 侧无新 crate，复用 `tokio::sync::broadcast` + `tokio::task::JoinSet`。
- **兼容**：HTTP API 路由 `list_sessions` 保持同步完整返回（HTTP 无事件通道）——通过为 HTTP 侧保留旧行为实现（见 design.md）。
- **测试**：Rust 新增 local.rs 集成测试（骨架返回 + 并发元数据推送）；前端保留 svelte-check 即可。

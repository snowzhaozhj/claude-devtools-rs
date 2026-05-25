## Why

跨进程 push event 的 payload 字段形态契约（`file-change` / `session-metadata-update` / `detected-error` / `sse-lagged` 等）当前散落在 `ipc-data-api`（schema owner，含 8+ 处 SHALL 句）、`http-data-api`、`sidebar-navigation`、`session-display`、`frontend-test-pyramid`、`notification-triggers` 六个 spec 内重复定义，违反 `openspec/SPEC_GUIDE.md` 的「外部协议单一 owner」原则。

PR #306 file-watching 试点（archive `spec-overhaul-file-watching-pilot`）的 `design.md::D-1/D-2` 已论证：新建 `push-events` capability 持有所有跨进程 push event payload schema，让 `ipc-data-api` 与 `http-data-api` 各自简化为 transport 桥契约，是该 9-PR 序列中唯一让 `ipc-data-api` 减负且让 PR 9 拆分自然落地的路径。本 PR 是该决策的落地，issue #303 9-PR 序列的 PR 2。

## What Changes

- **新增** `push-events` capability：单 owner 持有所有跨进程 push event 的 payload 字段形态契约（字段名 / camelCase / serde tag 约定 / 字段语义 / variant 枚举），含 `file-change` / `session-metadata-update` / `detected-error` / `sse-lagged` / `ssh-status-change` 等所有 `PushEvent` enum variant。
- **修改** `ipc-data-api`：移走 `file-change` payload schema 整段 SHALL（含 `sessionListChanged` / `camelCase` / `SseLagged` 形态契约 + `Tauri 转发 file-change 事件` / `file-change payload 是 camelCase` / `enriched session_list_changed` 等 Scenario），改为 transport 桥契约——「Tauri host SHALL bridge 各类 push event」，具体 payload 形态引用 `[[push-events]]`。
- **修改** `http-data-api`：SSE PushEvent payload 形态契约引用 `[[push-events]]`；HTTP transport 层细节（路径 / Content-Type / `lastEventId` 重连 / `sse_lagged` sentinel 何时发出 / `ensureSseReady` race）保留。
- **修改** `frontend-test-pyramid`：mockIPC listen event 名单 SHALL 来源引用 `[[push-events]]`，本 spec 仅断言 mockIPC 与之逐项对齐（不再硬编码 4 条事件名）。
- **修改** `notification-triggers`：`FileSignature` 自然恢复机制描述引用 `[[push-events::file-change]]`。
- **修改** `sidebar-navigation`：保留 listen 消费行为断言（"收到 X 时 → 调 Y" / "命中当前选中 → 拉新数据"等业务行为）；遇到直接复制字段名 / 字段语义的句子改为引用 `[[push-events::file-change]]` 或 `[[push-events::session-metadata-update]]`。
- **修改** `session-display`：同 `sidebar-navigation`，保留消费断言、字段定义引用 push-events。
- **同 PR 刷新** `scripts/spec-purity-baseline.txt`：propose 期加 `change/push-events-cap/*` 行；archive 期再删 + 加 `spec/push-events` 行（PR #306 已踩过坑，详 archive design.md::D-3）。

不动后端代码 / 测试，纯 spec 文档重写。

## Capabilities

### New Capabilities
- `push-events`: 跨进程 push event payload schema 单 owner——字段形态 / camelCase / serde tag / variant 枚举，覆盖 Tauri webview event 与 HTTP `/api/events` SSE 两条 transport 共享的 payload 形态契约

### Modified Capabilities
- `ipc-data-api`: 移走 file-change payload schema 整段 SHALL；改为 transport 桥契约引用 push-events
- `http-data-api`: SSE PushEvent payload 形态引用 push-events；HTTP transport 层细节保留
- `frontend-test-pyramid`: mockIPC listen event 名单引用 push-events，删除硬编码 4 条事件名
- `notification-triggers`: `FileSignature` 自然恢复描述引用 `[[push-events::file-change]]`
- `sidebar-navigation`: 字段名 / 字段语义直接复制处改为引用 push-events，消费行为断言保留
- `session-display`: 同 sidebar-navigation

## Impact

- **不动代码 / 测试**：纯 spec 文档重写，行为契约语义不变；archive 后主 spec 自动 sync。
- **lint baseline**：`scripts/spec-purity-baseline.txt` 同 PR 刷新两次（propose 期 + archive 期），防 ratchet 倒挂。
- **未来 PR**：本 PR archive 后，PR 3+ 的 `ssh-remote-context` / `configuration-management` / `sidebar-navigation` 改动可基于稳定的 `push-events` 主 spec 作为单一引用 owner；不再撞 schema 重复 owner。
- **架构例外**：主 spec Purpose 段直 edit（继承 PR #306 `design.md::D-3` 先例，OpenSpec 的 spec delta 架构不解析 Purpose section）。

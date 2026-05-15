## Why

Sidebar 当前的 `loadSessions(projectId, silent=true)` 在 file-change 推送或用户点击"有更新"按钮时只请求第一页（20 条）并整体替换 `sessions` 数组 + 重置 `sessionsNextCursor`，导致用户已翻页到 N（N > 20）条后，sessions 列表瞬间缩水到 20 余条，紧接着 `maybeLoadMoreSessions` 又把列表补回——session-count `{visibleSessions.length}/{totalSessions}` 来回跳变，scrollTop 同时失去原本指向的会话锚点，"点有更新跳转的位置不对"。

活跃 Claude 会话每秒可触发多次 file-change（CLAUDE.md "file-change 节流链"段），跳变频率与列表深度成正比，翻得越多越显眼。

## What Changes

- `Sidebar.svelte::loadSessions(projectId, silent=true)` SHALL 用现有的 `mergeSessions(prev, result.items, sort=true)` 合并第一页到现有 `sessions`，保留 prev 中超出第一页的尾部 sessions；SHALL NOT 重置 `sessionsNextCursor`，保留用户已翻到的分页位置
- 非 silent 路径（首次切 project 加载）行为不变：仍然 `sessions = result.items` + `sessionsNextCursor = result.nextCursor`
- 新增 vitest 单测覆盖 silent 合并语义（保留尾部 + 不重置 cursor + 复用 mergeSilentMetadata 保留 prev 元数据）

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `sidebar-navigation`：扩展现有 Requirement "会话元数据增量 patch"，新增 silent 刷新保留已翻页 sessions 与分页 cursor 语义；扩展 Requirement "Sidebar uses paginated current-project session loading"，明确 silent 刷新场景下点击"有更新"按钮不丢弃尾部分页

## Impact

- 代码：`ui/src/components/Sidebar.svelte`（silent 分支 ~5 行）+ 新增 `ui/src/lib/sessionMerge.ts`（抽出 `mergeSessions` / `mergeSilentMetadata` / `applySilentRefresh` 三个纯函数 helper，组件内引用改 import）
- 测试：`ui/src/lib/sessionMerge.test.ts`（新增，纯单测覆盖 silent 合并 + cursor 保留语义）
- 性能：silent 路径多一次 `Map` 构造 + 一次 `Array.sort`（N≈60 量级 ~360 比较），无可感差异
- 不破坏现有 IPC 协议；不动 Rust 后端

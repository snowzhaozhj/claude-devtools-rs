## Why

Codex 异构二审在 PR #37（`search-hydrate-lazy-chunks`）中找到 file-change × SearchBar 的预先存在 bug：当 SearchBar 处于可见 + 有 query 状态时，`SessionDetail.svelte` 的 file-change handler 调 `refreshDetail` 替换 `detail = d`，新增 chunk 不参与已发生的 `highlightMatches`，旧 `<mark>` 索引按旧总数循环导致 next / prev 跳到错误位置；新进入视口被 hydrate 的 chunk 也不携带 mark。窄场景但用户感知明显。该 bug 已在 `openspec/followups.md` 记录为待修，本 change 完成兜底。

## What Changes

- `ui/src/components/SearchBar.svelte::Props` 新增 `contentVersion?: number`：调用方在容器内容（chunk 列表 / 已渲染 chunk 文本）发生变化时递增该值，SearchBar SHALL 在 `visible && query && contentVersion 变化` 时自动重跑 `doSearch` 同步索引
- `ui/src/routes/SessionDetail.svelte` 新增 `searchContentVersion: number` 状态，在 `refreshDetail` 完成 `detail = d` 后递增，作为 `contentVersion` prop 传给 SearchBar
- `openspec/specs/ui-search/spec.md` 在 `Cmd+F 激活会话内搜索` Requirement 加 Scenario 规约 file-change 后 SearchBar 自动重搜行为契约

## Capabilities

### New Capabilities
（无 — 本 change 在既有 capability 内补 Scenario）

### Modified Capabilities
- `ui-search`：`Cmd+F 激活会话内搜索` Requirement 新增 Scenario，规约 SearchBar 与外部内容版本号的协作契约

## Impact

- 代码：`ui/src/components/SearchBar.svelte`（加 prop + `$effect`）、`ui/src/routes/SessionDetail.svelte`（加版本号状态 + 透传）
- 测试：vitest 覆盖 `contentVersion` 变化触发 doSearch 的行为
- 不影响：后端 IPC、Rust crate、Tauri command、`onBeforeSearch` hook（已落地）、其他调用 SearchBar 的视图（如未来加新调用方默认 `contentVersion = 0` 不递增即维持旧行为）

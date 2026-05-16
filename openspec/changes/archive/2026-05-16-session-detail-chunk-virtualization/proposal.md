## Why

SessionDetail 长会话当前把 `detail.chunks` 全量挂载为 DOM，用户即使在无更新时滚动也会触发大量布局、样式和 observer 工作，导致 CPU 超过桌面辅助工具预算。需要在主会话流引入 chunk 级 variable-height virtualization，从根因减少常驻 DOM 与滚动时主线程负载。

## What Changes

- 在 SessionDetail 主对话流中引入 chunk 级虚拟化：仅渲染可视窗口和 overscan 内的 chunk，窗口外用 spacer 保持滚动高度。
- 支持 variable-height：先按 chunk 类型估算高度，再通过 `ResizeObserver` 记录真实高度并修正 offset。
- 保持既有 SessionDetail 行为：lazy markdown、Mermaid、image lazy load、工具展开/收起、搜索、滚动到底、file-change 自动刷新、per-tab scroll 状态与 `openOrReplaceTab` 状态隔离不回退。
- 保留紧急回滚能力：前端常量关闭后退化为全量渲染旧行为。

## Capabilities

### New Capabilities

- 无。

### Modified Capabilities

- `session-display`: 增加 SessionDetail 主会话流虚拟化的性能与交互契约。

## Impact

- 影响 `ui/src/routes/SessionDetail.svelte` 主对话流渲染结构。
- 可能新增或扩展 `ui/src/lib/*virtual*.svelte.ts` 以支持动态高度虚拟化。
- 影响与滚动容器耦合的逻辑：lazy markdown root、搜索、自动刷新贴底、scrollTop 保存/恢复、工具展开导致的高度变化。
- 不改变 Rust 后端、Tauri IPC 字段或数据结构。

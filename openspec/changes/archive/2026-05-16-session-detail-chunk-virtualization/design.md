## Context

SessionDetail 当前在 `.conversation` 中直接 `{#each detail.chunks}` 全量渲染，长会话会让所有 chunk DOM、lazy markdown 占位、tool row、observer 和事件结构常驻。用户观察到详情页无更新时滚动 CPU 超过 10%，根因是滚动路径仍需处理全量 DOM 的 layout/paint/observer 成本。

现有 `ui/src/lib/virtualList.svelte.ts` 仅支持固定 `itemHeight` 的平铺列表，适合 Sidebar 类等高项；SessionDetail chunk 高度由 markdown、工具展开、image、subagent、Mermaid 等决定，必须支持 variable-height。

## Goals / Non-Goals

**Goals:**

- SessionDetail 主会话流只挂载可见窗口和 overscan 内的 chunk，显著降低长会话滚动 CPU。
- 支持 chunk 高度动态变化：markdown 懒渲染、工具展开/收起、image 加载、Mermaid 渲染后都能修正高度。
- 保持现有交互：搜索、滚动到底、自动刷新贴底、per-tab scroll 保存/恢复、`openOrReplaceTab` 状态隔离、lazy markdown 与工具 lazy output。
- 提供一行常量回滚到全量渲染，便于发现严重 UI 回归时快速切回。

**Non-Goals:**

- 不改变 Rust 后端、Tauri IPC、`SessionDetail` 数据结构或 chunk-building 语义。
- 不重写 Read/Edit/Write/Bash 等工具查看器。
- 不在本 change 内做 tool 内部行级虚拟化；工具详情内部大文本性能继续由既有 lazy/heavy-rendering 规则约束。
- 不优化全局搜索算法；只保证 SessionDetail 现有 DOM 搜索语义不被虚拟化破坏。

## Decisions

### D1: 新增动态高度虚拟化控制器，而不是复用固定高度 `virtualList`

采用新的 `createDynamicVirtualizer`（命名可在实现时微调）管理 `count`、`estimateSize(index)`、`overscanPx`、`scrollTop`、`viewportHeight`、实测高度 map、可见 range、top/bottom spacer 与 `scrollToEnd()`。

候选方案：
- 直接复用 `virtualList.svelte.ts`：实现量小，但固定高度会在 markdown/tool 展开后出现 offset 漂移和跳动。
- 引入第三方虚拟列表库：功能完整，但会新增依赖、适配 Svelte 5 runes 与现有滚动/搜索/lazy root 的成本不可控。
- 自研最小动态虚拟化：只覆盖 SessionDetail 所需 API，风险和依赖面可控。

选择自研最小动态虚拟化，保留现有 fixed virtualList 给等高列表使用。

### D2: 以 chunk 为虚拟化粒度，而不是 message/tool/markdown 块粒度

主滚动容器按 `detail.chunks` 切片渲染，每个可见 chunk 包一层 measured row。工具列表、markdown、subagent 等子结构保持当前组件层次。

候选方案：
- tool/message 级虚拟化：DOM 更少，但会破坏 AIChunk 内部时序、展开状态和 lazy output，实施风险高。
- chunk 级虚拟化：DOM 降幅足够大，改动边界集中，保留现有子组件行为。

选择 chunk 级，后续若工具详情内部仍有瓶颈再单独针对 tool viewer 优化。

### D3: 搜索激活时临时全量渲染，保证现有 DOM 搜索正确

现有 `SearchBar` 通过容器 DOM `TreeWalker` 搜索并高亮文本。虚拟化后远端 chunk 不在 DOM 中，直接沿用会漏结果。第一阶段在搜索面板打开或搜索 query 非空时禁用虚拟化并调用 lazy markdown `flushAll()`，保持当前全文 DOM 搜索语义。

候选方案：
- 重写搜索为数据级索引 + `scrollToIndex`：性能更好，但改动范围跨 `ui-search` 行为，风险超过本 change。
- 搜索时全量渲染：搜索是显式用户操作，可接受短时成本，能避免功能回退。

选择搜索时全量渲染；后续可另开 change 做数据级搜索。

### D4: 滚动到底与贴底刷新改走虚拟化总高度能力

虚拟化启用时 `scrollHeight` 由 spacer 与 visible rows 共同构成，但仍可通过控制器的 `scrollToEnd()` 使用估算总高度定位到底部。file-change 刷新前仍按容器 `scrollTop + clientHeight >= scrollHeight - 16` 判定 pinned-to-bottom；刷新后调用 `scrollToEnd()`，非 pinned 状态不主动改 `scrollTop`。

候选方案：
- 继续直接写 `container.scrollTop = container.scrollHeight`：对全量 DOM 和 spacer 都能工作，但测试与意图不清晰。
- 封装 `scrollToEnd()`：统一处理 virtualized/full 两种路径，便于后续调整估算总高度。

选择封装能力，降低 SessionDetail 内部分支。

### D5: 展开状态保留现状但新增稳定 chunk key 供测量缓存

测量缓存使用 `chunkKey`（优先 uuid / response uuid / fallback index）关联高度。现有 `expandedChunks` / `expandedItems` 若仍使用 index key，本 change 不强制迁移，避免同时改过多状态语义；但 `openOrReplaceTab` 已通过 tab/session guard 隔离旧状态，虚拟化测量缓存必须在 `sessionId` 或 chunk key 列表变化时 reset。

候选方案：
- 一次性把所有展开 key 迁移到 chunkKey：更稳，但触及大量工具项 key 和旧状态兼容。
- 仅虚拟化测量使用 chunkKey：满足性能目标，减少行为变化。

选择后者；若实现中发现 index key 直接导致 bug，再在同 change 内做最小迁移并补测试。

## Risks / Trade-offs

- 搜索时全量渲染会临时恢复高 DOM 成本 → 仅在搜索 UI 激活时发生，保持功能正确优先；后续可做数据级搜索优化。
- variable-height 估算不准可能导致滚动条跳动 → 使用 `ResizeObserver` 实测更新，并在用户 pinned-to-bottom 时补偿到底部。
- Lazy markdown / Mermaid / image 渲染后高度二次变化 → measured row 包住完整 chunk，任何子树高度变化都由 ResizeObserver 回写。
- 虚拟化会频繁 mount/unmount chunk → chunk key 必须稳定，lazy markdown 已渲染态按 chunkKey 保留；测试覆盖滚动离开再回来。
- overscan 太小会出现滚动空白，太大则 DOM 过多 → 初始使用像素 overscan（如 800-1200px），用浏览器 smoke 调整。

## Migration Plan

1. 新增动态虚拟化控制器与单元测试。
2. 在 SessionDetail 主 conversation 接入 measured rows、spacer、回滚常量和搜索时全量渲染分支。
3. 更新/新增 SessionDetail 相关 vitest/e2e，覆盖大 chunks 仅渲染窗口、搜索远端 chunk、展开后高度更新、贴底刷新。
4. 用 mock fixture 浏览器 smoke 长列表滚动 CPU 与交互。
5. 若发现不可控回归，保持 change 为设计 PR 或把回滚常量默认关闭，不合入半成品。

## Open Questions

- 现有 mock fixture 是否已有足够长的 session；若没有，需要在测试 fixture 中构造长 chunks 数据。
- 搜索全量渲染的触发条件以 `searchVisible` 为准还是以 query 非空为准；实现时优先选择不漏结果的条件。

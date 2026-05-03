## Why

change `session-detail-lazy-render` 把 chunk 内 markdown 改为视口懒渲染后产生副作用：应用内 `SearchBar`（Cmd+F）通过 DOM `TreeWalker` 高亮匹配项，但视口外的 chunk 仅有占位 div，无 markdown 文本节点 — 用户在大会话中搜索时，视口外的匹配项被静默跳过，匹配数偏少甚至为 0，next / prev 导航也无法跳到未渲染段。该副作用已记录在 `openspec/followups.md` 的 `lazy markdown 副作用：搜索高亮无法命中未渲染 chunk`，等真实痛点出现再修；现确认为发版前需关闭的回归项。

## What Changes

- `ui/src/lib/lazyMarkdown.svelte.ts::LazyMarkdownObserver` 接口新增 `flushAll()` 方法：把当前所有 pending 占位元素按注册顺序同步 `renderInto`，清空 `pending` map，从 `IntersectionObserver` `unobserve` 已渲染元素
- `LAZY_MARKDOWN_ENABLED = false` 回滚分支也 SHALL 提供 `flushAll()`（no-op 即可，因为该分支下所有元素首帧已同步渲染）
- `ui/src/components/SearchBar.svelte::doSearch` 在调用 `highlightMatches` 之前 SHALL 先触发外部传入的 `onBeforeSearch` 回调（语义为"准备容器供 DOM 文本扫描"），由调用方在回调里 hydrate 全部占位
- `ui/src/routes/SessionDetail.svelte` SHALL 把 `lazyObserver.flushAll` 作为 `onBeforeSearch` 传给 `SearchBar`
- `openspec/specs/ui-search/spec.md` 在 `Cmd+F` 相关 Requirement 加 Scenario：搜索激活 SHALL 触发 conversation 容器内全部 lazy 占位的强制渲染，使匹配项数与全文一致
- `openspec/specs/session-display/spec.md` 在 `Lazy markdown rendering for first paint performance` Requirement 加 Scenario：lazy markdown 控制器 SHALL 暴露 `flushAll()` 同步把所有 pending 占位渲染为真实 HTML，供搜索 / 打印等需要全文 DOM 的场景调用

## Capabilities

### New Capabilities
（无 — 本 change 在既有 capability 内补 Scenario）

### Modified Capabilities
- `ui-search`：`Cmd+F 激活会话内搜索` Requirement 新增 Scenario，规约搜索激活与 lazy markdown 全量 hydrate 的协作契约
- `session-display`：`Lazy markdown rendering for first paint performance` Requirement 新增 Scenario，规约 lazy markdown 控制器对外暴露的 `flushAll()` 行为契约

## Impact

- 代码：`ui/src/lib/lazyMarkdown.svelte.ts`（加 `flushAll`）、`ui/src/components/SearchBar.svelte`（调用 hook）、`ui/src/routes/SessionDetail.svelte`（透传 hook）
- 测试：vitest 覆盖 `flushAll` 行为 + SearchBar 对 hook 的调用顺序；Playwright 覆盖"未渲染段含唯一关键词，搜索后命中"用户故事
- 性能：用户首次按 Cmd+F 触发一次性渲染，大会话场景一次性付出 marked / highlight.js / DOMPurify / mermaid 的全量成本；首屏与日常滚动不受影响
- 不影响：后端 IPC、Rust crate、Tauri command、`LAZY_MARKDOWN_ENABLED` 回滚开关、subagent trace lazy load（独立组件，无搜索集成）、浏览器原生 Cmd+F（依然不命中，followups 单列）

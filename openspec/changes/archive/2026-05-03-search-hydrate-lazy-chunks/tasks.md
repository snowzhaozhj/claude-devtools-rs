## 1. lazyMarkdown 控制器加 `flushAll`

- [x] 1.1 把 `ui/src/lib/lazyMarkdown.svelte.ts` 内 `pending` 从 `WeakMap<Element, ...>` 改为 `Map<Element, ...>`，便于枚举 entries
- [x] 1.2 在 `LazyMarkdownObserver` interface 上加 `flushAll(): void` 方法签名
- [x] 1.3 enabled 分支实现 `flushAll`：遍历 `pending.entries()` 调 `renderInto(el, text, onRendered)`，`io.unobserve(el)`，最后 `pending.clear()`
- [x] 1.4 disabled 分支（`LAZY_MARKDOWN_ENABLED = false`）实现 `flushAll`：no-op
- [x] 1.5 `disconnect()` 兜底加 `pending.clear()` 避免 Map 内存泄漏

## 2. SearchBar 加 hook

- [x] 2.1 在 `ui/src/components/SearchBar.svelte::Props` 上加 `onBeforeSearch?: () => void`
- [x] 2.2 `doSearch` 函数体在 `clearHighlights(containerEl)` 之后、`highlightMatches(containerEl, query)` 之前调用 `onBeforeSearch?.()`
- [x] 2.3 保留 `onBeforeSearch` 同步语义（不 await Promise），与 `flushAll` 同步签名一致

## 3. SessionDetail 透传

- [x] 3.1 `ui/src/routes/SessionDetail.svelte` 把 `lazyObserver.flushAll` 透传给 `<SearchBar>`：`onBeforeSearch={() => lazyObserver?.flushAll()}`
- [x] 3.2 确认 `lazyObserver` 在组件销毁前可用（与 `containerEl` 生命周期一致）

## 4. 单元测试（vitest）

- [x] 4.1 新建 `ui/src/lib/lazyMarkdown.test.ts`：测 `flushAll` 把 N 个 pending 元素全部 hydrate，断言 `el.dataset.rendered === "1"` + `el.innerHTML` 非空
- [x] 4.2 同上，测 `flushAll` 幂等性：连续调用两次不报错，第二次为 no-op
- [x] 4.3 同上加 `flushAll` 触发 `onRendered` 回调测试（mermaid 后处理钩子）+ `data-rendered` 元素跳过 + `disconnect` 后 pending 清空 共 6 个用例
- [~] 4.4 SearchBar 调用顺序测试：项目当前无 svelte 组件级测试基建（`ui/src/components/__tests__/` 为空），改用 e2e 兜底；纯函数提取重构超出本 change 范围

## 5. 端到端测试（Playwright，可选）

- [~] 5.1 跳过：现有 `ui/tests/e2e/` 基建未覆盖 SearchBar 输入流程，不在本 change 范围内引入新 e2e
- [x] 5.2 由 4.x vitest 单测 + 6.2 手动 smoke 兜底

## 6. 验证与归档

- [x] 6.1 `npm run check --prefix ui`（543 文件 0 错）+ `npm run test:unit --prefix ui`（77 通过 1 跳过）全绿
- [ ] 6.2 `cargo tauri dev` 手动 smoke：打开大会话 → Cmd+F 输入视口外关键词 → 验证命中数与全文一致 + scrollIntoView 正确
- [x] 6.3 `openspec validate search-hydrate-lazy-chunks --strict` 通过
- [ ] 6.4 commit + PR + codex 二审（行为契约改动）
- [ ] 6.5 PR 合并前在同一分支 `openspec archive search-hydrate-lazy-chunks -y` 把 delta sync 回主 spec 作为 PR 最后一个 commit
- [ ] 6.6 `openspec/followups.md` 标记 `lazy markdown 副作用：搜索高亮无法命中未渲染 chunk` 为 ✅ 已修复，引用 change `search-hydrate-lazy-chunks`

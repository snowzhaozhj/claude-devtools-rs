## 1. SearchBar 加 contentVersion prop + 自动重搜

- [x] 1.1 在 `ui/src/components/SearchBar.svelte::Props` 上加 `contentVersion?: number`，默认 0（向后兼容）
- [x] 1.2 加 `$effect`：依赖 `contentVersion`，gate 为 `visible && query`，触发时调 `doSearch()`
- [x] 1.3 初次 mount 防御：`doSearch` 内 `if (!query) return` 已兜底，无需额外处理

## 2. SessionDetail 透传 + 递增

- [x] 2.1 `ui/src/routes/SessionDetail.svelte` 加 `let searchContentVersion = $state(0)`
- [x] 2.2 `refreshDetail` 函数内 `try { ... detail = d; setCachedSession(...); searchContentVersion++; ... }`
- [x] 2.3 `<SearchBar contentVersion={searchContentVersion} ... />` 透传

## 3. 单元测试（vitest）

- [~] 3.1 跳过：项目当前无 svelte 组件级 vitest 基建（`ui/src/components/__tests__/` 为空），`$effect` 依赖追踪是 Svelte 5 runtime 行为，纯函数无法覆盖
- [~] 3.2 不抽纯函数：`shouldRerunSearch(visible, query) === visible && query !== ''` 太琐碎，抽出反而引入不必要的间接层
- [x] 3.3 由 4.2 手动 smoke 兜底；spec scenario 锁定行为契约

## 4. 验证与归档

- [x] 4.1 `npm run check --prefix ui`（543 文件 0 错）+ `npm run test:unit --prefix ui`（77 通过 1 跳过）全绿
- [ ] 4.2 `cargo tauri dev` 手动 smoke：打开 ongoing session → 按 Cmd+F 输入查询 → 等 file-change 自动刷新 → 验证 totalMatches 更新
- [x] 4.3 `openspec validate search-rerun-on-content-change --strict` 通过
- [ ] 4.4 `openspec archive search-rerun-on-content-change -y` sync delta 回主 spec
- [ ] 4.5 `openspec/followups.md` 标记 `file-change 自动刷新后 SearchBar mark 索引过期` 为 ✅ 已修复，引用 change `search-rerun-on-content-change`

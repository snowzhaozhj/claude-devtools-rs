## 1. 上下文提取工具

- [x] 1.1 新建 `ui/src/lib/contextExtractor.ts`，定义 `ContextEntry` 类型（category、label、preview、estimatedTokens）
- [x] 1.2 实现 `extractContext(chunks): ContextEntry[]`，从 system/AI/user chunks 提取 4 类上下文
- [x] 1.3 实现 `groupByCategory(entries): Map<string, ContextEntry[]>`，按类别分组

## 2. ContextPanel 组件

- [x] 2.1 新建 `ui/src/components/ContextPanel.svelte`，320px 右侧边栏，header + 分组列表
- [x] 2.2 每个类别可折叠/展开，每个条目显示 label + token badge + 可点击展开查看完整内容
- [x] 2.3 Header 显示总注入数 + 总 token 估算 + 关闭按钮

## 3. 集成到 SessionDetail

- [x] 3.1 `SessionDetail.svelte` 添加 `contextPanelVisible` 状态
- [x] 3.2 top-bar 的 Context badge 改为可点击 toggle 按钮
- [x] 3.3 conversation 区域和 ContextPanel 横向 flex 布局（conversation flex:1 + ContextPanel 320px 条件渲染）

## 4. 验证

- [x] 4.1 `npm run check --prefix ui` 类型检查通过
- [ ] 4.2 `cargo tauri dev` 视觉验证：点击 Context badge 打开/关闭面板，查看分类内容

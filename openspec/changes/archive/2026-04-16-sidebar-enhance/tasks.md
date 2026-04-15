## 1. Sidebar Store

- [x] 1.1 创建 `ui/src/lib/sidebarStore.svelte.ts`：模块级 `$state` 管理 sidebarWidth（默认 280，范围 200~500）、pinnedByProject（Record<string, string[]>）、hiddenByProject、showHidden
- [x] 1.2 导出 API：`getSidebarWidth`/`setSidebarWidth`、`isPinned`/`togglePin`/`getPinnedIds`、`isHidden`/`toggleHide`/`getHiddenCount`、`getShowHidden`/`toggleShowHidden`

## 2. 右键菜单组件

- [x] 2.1 创建 `SessionContextMenu.svelte`：fixed 浮层，5 个菜单项（新标签页打开、置顶/取消、隐藏/取消、复制 ID、复制恢复命令）
- [x] 2.2 viewport 边缘 clamping（MENU_WIDTH=220, MENU_HEIGHT=240，8px 边距）
- [x] 2.3 关闭机制：点击外部 mousedown + Escape 键
- [x] 2.4 复制操作：navigator.clipboard + 600ms "已复制!" 反馈后自动关闭

## 3. Sidebar 集成

- [x] 3.1 宽度拖拽：右边缘 resize handle（position: absolute, 5px 宽），mousedown→mousemove→mouseup 全局监听，拖拽时 cursor: col-resize + userSelect: none，hover/active 蓝色高亮
- [x] 3.2 动态宽度：sidebar style:width/style:min-width 绑定 store，移除 CSS 硬编码 280px
- [x] 3.3 右键菜单触发：session item 加 oncontextmenu handler，传入位置和 session 数据
- [x] 3.4 Pin 分区：pinnedSessions 从 visibleSessions 中分离，渲染在日期分组之前，显示 PINNED 标签 + 蓝色 pin SVG 图标
- [x] 3.5 Hide 过滤：默认隐藏 isHidden 的会话；hiddenCount > 0 时 filter bar 显示眼睛图标切换按钮；被隐藏的会话在 showHidden 模式下以 50% opacity 显示
- [x] 3.6 `{@const ctx = ctxMenu}` 安全捕获 context menu 闭包引用

## 4. Delta Spec

- [x] 4.1 更新 sidebar-navigation spec：新增 Pin/Hide/右键菜单/宽度调整 Requirements 和 Scenarios

## 5. 验证

- [x] 5.1 `npm run check --prefix ui` 类型检查通过（0 错误）

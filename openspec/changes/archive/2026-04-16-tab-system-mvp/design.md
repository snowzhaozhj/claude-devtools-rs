## Context

当前 Rust 版前端是单会话模式：`App.svelte` 维护 `selectedSessionId` 单一状态，Sidebar 点击 session 直接替换 Main 区域。原版 claude-devtools 的 Tab 系统支持多 Pane、DnD、per-tab 状态隔离等完整功能。本次 MVP 只实现核心 tab 管理能力。

现有前端状态全部用 Svelte 5 `$state` rune 在组件内管理，无全局 store。Tab 系统需要跨组件共享状态（TabBar、Sidebar、SessionDetail 都需要访问 tab 列表），因此需要引入模块级响应式 store。

## Goals / Non-Goals

**Goals:**
- 支持多 tab 打开不同 session，快速切换无需重新加载
- TabBar 显示已打开的 tab，支持切换和关闭
- Sidebar 点击 session 自动复用已打开的 tab 或新建
- Per-tab UI 状态隔离（展开/折叠、搜索、Context Panel）
- Per-tab session 数据缓存（切换时 zero-latency）

**Non-Goals:**
- DnD 拖拽排序 tab
- 多 Pane 分屏并排查看
- Tab 右键菜单（关闭其他、分屏等）
- Tab 状态持久化（重启恢复）
- Tab 重命名
- Dashboard / Settings / Notifications tab 类型（只支持 session tab）

## Decisions

### 1. 状态管理方案：模块级 `$state` + 导出函数

**选择**：在 `ui/src/lib/tabStore.ts` 中用模块级 `$state` rune 定义响应式状态，导出 getter/action 函数。

**替代方案**：
- Svelte store（`writable`）：旧 API，Svelte 5 推荐 runes
- Context API：需要 provider 组件包裹，不适合跨层级共享
- 第三方库（如 zustand 的 Svelte 适配）：引入不必要依赖

**理由**：Svelte 5 的模块级 `$state` 在 `.svelte.ts` 文件中天然支持跨组件响应式共享，无需额外依赖，与现有代码风格一致。

### 2. Tab 数据模型

```typescript
interface Tab {
  id: string;           // crypto.randomUUID()
  sessionId: string;
  projectId: string;
  label: string;        // session 标题（截断 50 字符）
  createdAt: number;    // Date.now()
}
```

只支持 `session` 类型 tab，不引入 `type` 字段（MVP 不需要 dashboard/settings/notifications tab）。

### 3. Per-tab 状态隔离方案

**选择**：在 `tabStore` 中维护 `Map<tabId, TabUIState>` 和 `Map<tabId, SessionDetail>`。

```typescript
interface TabUIState {
  expandedChunks: Set<number>;
  expandedItems: Set<string>;
  searchVisible: boolean;
  contextPanelVisible: boolean;
  scrollTop: number;
}
```

切换 tab 时：
1. 保存当前 tab 的 UI 状态 + 滚动位置
2. 恢复目标 tab 的 UI 状态
3. 若目标 tab 有缓存的 SessionDetail，直接使用；否则调用 API 加载

**理由**：比原版的 Zustand slice 模式简单，适合 Svelte 5 runes 风格。状态集中在一个 store 模块中，易于理解和维护。

### 4. SessionDetail 改造方案

**选择**：SessionDetail 不再自行管理 `expandedChunks`/`expandedItems` 等 UI 状态，改为从 tabStore 读写。组件保持无状态（stateless），所有持久状态由 tabStore 管理。

**影响**：SessionDetail 的 `$effect` 需要改为通过 tabStore action 操作，而非直接修改本地 `$state`。

### 5. 布局结构

```
app-layout (flex: horizontal)
├── Sidebar (width: 280px，不变)
└── main-area (flex: 1, flex-direction: column)
    ├── TabBar (height: 36px, 固定)
    │   ├── tab-list (水平滚动)
    │   │   └── tab-item × N (点击切换，X 关闭)
    │   └── new-tab 按钮（占位，MVP 可选）
    └── main-content (flex: 1)
        ├── SessionDetail (有活跃 tab)
        └── empty-state (无 tab)
```

### 6. Sidebar 集成

Sidebar 的 `onSelectSession` 回调改为调用 `tabStore.openTab(sessionId, projectId, label)`：
- 检查是否已有该 sessionId 的 tab → 有则 `setActiveTab`
- 没有 → 创建新 tab 并设为 active

Sidebar 高亮逻辑从 `selectedSessionId` 改为 `activeTab?.sessionId`。

## Risks / Trade-offs

- **[内存占用]** Per-tab 缓存所有 SessionDetail 数据 → 大量 tab 时内存增长。缓解：MVP 阶段不处理，后续可加 LRU 淘汰。
- **[滚动位置恢复精度]** 保存/恢复 scrollTop 可能因内容高度变化而不精确。缓解：MVP 阶段可接受近似恢复。
- **[SessionDetail 重构范围]** 将 UI 状态外提到 tabStore 需要较大改动。缓解：分步进行，先建 store，再逐步迁移状态。

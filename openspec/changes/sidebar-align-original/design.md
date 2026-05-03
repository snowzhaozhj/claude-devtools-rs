## Context

原版 Electron SidebarHeader 是双行布局：Row 1 = 项目名 + 折叠按钮（`PanelLeft` icon）；Row 2 = worktree 选择器（`GitBranch` icon + worktree name + ChevronDown）。Row 1 的折叠按钮配合 `Cmd+B` 快捷键能把 sidebar 整体收起，让 chat 区域占满全宽；Row 2 给用户传递"当前在哪个 git 分支工作"的一阶信息。

本端口当前 SidebarHeader 是单行（项目名 + 下拉箭头），没有折叠按钮也没有分支栏。底层数据已具备：`cdt-core::Worktree { git_branch }`、`cdt-core::Message::git_branch` 都已在；只是 IPC 表面没有透出 `git_branch`，前端拿不到。`cdt-core::Message::git_branch` 在 JSONL 每行都有，原版 `sessionExporter.ts:304` 取值方式 = 解析期间最后一条非空 `git_branch`。

## Goals / Non-Goals

**Goals:**

- SidebarHeader 加只读 git 分支栏，对齐原版 Row 2 视觉位置（不引入 worktree 切换）
- `SessionSummary` 暴露 `git_branch: Option<String>`，由现有 `session-metadata-update` 通道异步 patch
- SidebarHeader 加折叠按钮 + `Cmd+B` 快捷键，折叠后 sidebar 隐藏 + TabBar 左侧出现展开按钮
- 折叠状态在 `sidebarStore.svelte.ts` 持久化（内存级，与 sidebar 宽度同维度）

**Non-Goals:**

- 不实现 worktree 选择器（不引入 multi-worktree 切换 UX）
- 不引入新 IPC command；不动 `list_sessions` / `get_session_detail` 调用语义
- 不展示在每个 SessionItem 行内（用户已选 (b) 方案：单栏）
- 折叠状态不入 `cdt-config`（保持纯内存 per-session）
- 不在 sidebar 折叠时保留窄轨道（直接整宽度归零）

## Decisions

### D1: `git_branch` 字段挂在 `SessionSummary` 还是新 IPC？

**选择**：挂 `SessionSummary` + `SessionMetadataUpdate`，复用现有 broadcast 通道。

**候选**：
- (a) 挂 `SessionSummary` + `SessionMetadataUpdate`
- (b) 新增 `get_project_git_branch(projectId)` IPC
- (c) 挂 `ProjectInfo`，作为 project-level 字段

**取舍**：
- (a) 与现有"骨架 + 元数据增量 patch"模型对齐，无需新 IPC，前端在已有 `session-metadata-update` listener 多读一字段；切 session 时 `git_branch` 跟随 active session 自然变化
- (b) 多一次 IPC 往返；project-level 单值无法表达"不同 session 不同 branch"
- (c) `git_branch` 是 per-session 属性，挂 ProjectInfo 语义错

### D2: 后端取哪一条 `git_branch`？

**选择**：解析过程中保留**最后一条非空** `git_branch`。

**候选**：
- (a) 最后一条非空（原版 `sessionExporter.ts` 行为）
- (b) 第一条非空
- (c) 出现频率最高的

**取舍**：
- (a) 反映"会话当前所在分支"，与原版 `session.gitBranch` 语义对齐
- (b) 反映会话开始时分支，对终态判断不直观
- (c) 用户 checkout 多次时不稳定，且需 hashmap 计数

实现位置：`LocalDataApi::list_sessions` 的后台 JoinSet 任务里，调用 `cdt_parse::parse_file` 后扫一遍 `parsed.message.git_branch`，记录最后一条 `Some(...)`。

### D3: UI 显示规则——activeSessionId fallback 到 sessions[0]？

**选择**：active 优先，无 active 回退最近 session（`sessions[0]`，列表已按 timestamp desc）。

**候选**：
- (a) active 优先，无 active 回退 sessions[0]
- (b) 始终用 sessions[0]
- (c) 无 active 不显示

**取舍**：
- (a) 用户聚焦某 session 时显示该 session 的分支；初次切 project 时给主分支线索
- (b) 切到不同 session 时分支栏不更新，与"反映当前 active session 工作分支"语义不符
- (c) 切 project 后该栏空白，UX 差

### D4: 无 `git_branch` 时该栏渲染策略

**选择**：完全不渲染（不留空位、不显示 `--` 占位）。

**理由**：
- 非 git 项目（`Message::git_branch` 全为 `None`）强行占位会让用户误以为加载未完
- 该栏不影响其他元素布局（仅在有 branch 时插入），sidebar 整体视觉一致

### D5: SidebarHeader 把 `activeSessionId` 与 `sessions` 信息从哪拿？

**选择**：从 `Sidebar.svelte` 通过 props 显式传入。

**候选**：
- (a) Sidebar 传 props
- (b) SidebarHeader 直接 import `tabStore` 拿 `activeTabId` + 读 sessions 列表
- (c) 在 `sidebarStore.svelte.ts` 加 derived

**取舍**：
- (a) 与现有 props-driven 模式一致；测试时 mock 简单
- (b) SidebarHeader 当前不依赖 store，引入会增加耦合
- (c) sidebarStore 当前只管 width/pin/hide，不应跨入 session 数据域

### D6: 折叠状态存哪里？

**选择**：`sidebarStore.svelte.ts` 的模块级 `$state`（内存级，进程内）。

**候选**：
- (a) 模块级 `$state` 内存级（与 `getSidebarWidth` 同模式）
- (b) 持久化到 `cdt-config`（跨 session 保留）
- (c) `localStorage`（跨重启保留但不进 backend）

**取舍**：
- (a) 与现有 sidebar 宽度同维度（宽度也是内存级，重启回归默认 280px）；改动最小
- (b) 引入新 IPC 调用 + config schema 变化，超出本 change 范围
- (c) 多一种持久化层，与已有"内存级"模型不一致

承认 trade-off：用户重启应用后 sidebar 重新展开。如需跨重启持久化，下一个 change 把 width + collapsed 一并 promote 到 cdt-config 即可。

### D7: 折叠后展开入口放哪里？

**选择**：TabBar 最左侧加 `PanelLeft` icon 按钮。

**候选**：
- (a) TabBar 最左侧（与原版 `TabBar.tsx:280` 一致）
- (b) 屏幕左上角浮层固定按钮
- (c) 仅靠 Cmd+B 快捷键，无 UI 按钮

**取舍**：
- (a) 与原版位置一致，TabBar 已经横跨顶部，加 leading 按钮自然
- (b) 浮层管理复杂、与现有 TabBar/Header 布局冲突
- (c) 不支持只用鼠标的用户，UX 缺陷

### D8: 折叠态 sidebar 渲染策略

**选择**：折叠时不渲染 `<aside class="sidebar">`（条件渲染），不留窄轨道。

**候选**：
- (a) `{#if !collapsed}<Sidebar />{/if}`
- (b) `width: 0` + `overflow: hidden`
- (c) 留 `48px` 窄轨道仅显示折叠按钮

**取舍**：
- (a) DOM 完全卸载，资源占用最小；展开时 sidebar 重建（Sidebar 内部已通过骨架快速加载缓解视觉抖动）
- (b) 0 宽度 sidebar 仍占 DOM 与 ResizeObserver；潜在副作用
- (c) 与原版"完全隐藏"行为不一致

承认 trade-off：(a) 切换时 Sidebar 重建会重新触发 `listSessions`。但 sessions 拉取已有骨架快速加载，且元数据增量 patch 不会从零开始（后端 broadcast 在线）。

### D9: 快捷键监听位置

**选择**：`App.svelte` 顶层 `onMount` 监听 `keydown`。

**候选**：
- (a) `App.svelte` 顶层
- (b) 新建 `useKeyboardShortcuts.svelte.ts` store / runes 模块
- (c) `Sidebar.svelte` 内部（折叠后被卸载就失效）

**取舍**：
- (a) 全局快捷键自然在 App 层；本端口当前还没有快捷键基础设施，单点起步合理
- (b) 引入新模块抽象超前，等第二个快捷键再抽象
- (c) (a) 比 (c) 安全（折叠后仍能触发展开）

## Risks / Trade-offs

- **后端 metadata scan 多读 git_branch 字段** → scan 已遍历所有行算 `messageCount`，多读一字段几乎零成本
- **fixture 改动** → fixtures 已有 `metadata.gitBranch`（`multi-project-rich.ts:245`），需把 SessionSummary fixture 也补 `gitBranch` 字段保持 IPC 形态一致
- **active session 切换抖动** → 切 session 时分支栏跟随变化，是预期行为；通过 SidebarHeader 不在 sessions[] 整体重渲时重建（依赖现有 props-driven 静态结构）缓解
- **折叠后 sidebar 卸载触发 sessions 重拉** → 展开时 `listSessions` 重新发一次；骨架快速加载 + 元数据缓存 mitigate；如果实测体验差再 promote 到方案 (b)（width:0 保留 DOM）
- **快捷键冲突** → `Cmd+B` / `Ctrl+B` 在 macOS 下是常见 markdown bold；本应用无 markdown 编辑场景，且原版同样用 `Cmd+B`，沿用即可

## Migration Plan

不涉及数据迁移。前端老 bundle 收到带 `gitBranch` 的新 IPC payload 时 TS interface 多一字段，旧代码不读不影响渲染；后端老 caller（HTTP server）继续通过 `list_sessions_sync` 拿到含 `gitBranch` 的 SessionSummary，无需改动。

## Open Questions

无。

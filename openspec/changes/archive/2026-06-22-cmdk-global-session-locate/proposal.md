## Why

Cmd+K 命令面板的会话区当前**只搜当前选中项目**（`filteredSessions` 在 `!selectedProjectId` 时直接返回空），且有 query 时走后端组内正文全文搜索。用户已能"复制 Session ID"（`SessionMetaMenu` / `Sidebar` 右键），但拿到一个完整 sessionId 后**无法在应用里跨项目定位回那个会话**——不知道它属于哪个项目，只能逐个展开侧栏肉眼比对 8 位前缀。这让"复制 id"成了只有出口、没有回流的半截能力。

完整的"全局边打边匹配 title/正文"形态（形态 Y）需要全部会话的 title 进内存，而 title 靠 `MetadataCache`（LRU 2000）+ 仅在分页浏览时触发的后台扫描，无全量预热——搜索时补 title 会触发数百次 jsonl 读取，违反 idle CPU / 冷路径性能预算。因此本 change 只交付**零文件 I/O 的全局 sessionId 定位**（形态 X），把全局 title 搜索的基础设施留作后续 change。

## What Changes

- Cmd+K 会话区在有 query 时，**新增一路"全局 sessionId 子串定位"**：遍历 `projectDataStore` 内存快照里所有 `RepositoryGroup.worktrees[].sessions[]`（已是全量 sessionId、`list_repository_groups` 直接返回，无新 IPC、无文件 I/O），匹配 sessionId 子串，跨全部项目，不依赖 `selectedProjectId`。
- **保留**当前"选中项目时的组内正文全文搜索"（`searchGroupSessions`），不回归；两路结果合并去重。
- 全局命中条目的 title 采用 **best-effort**：仅当已加载/已缓存时显示，未命中则显示 `sessionId` 前缀 + 项目名定位；**不触发任何 metadata scan**（不复用 `listGroupSessions` 的 miss-scan 路径）。
- 修复改全局后必然暴露的既有缺陷：
  - 已打开面板持有 `projects` 局部快照、不随 `projectDataStore` 刷新更新（stale / ghost 会话）；
  - 前端裸遍历不继承 `dedup_sessions_across_worktrees`（同 sessionId 跨 worktree 重复显示）；
  - `openSession` 跨项目归属（确保按命中条目自身 `projectId` 打开，不打开错 scope）；
  - 海量命中的最小 query 长度门槛 + 稳定排序 + 截断语义；
  - 现有 query `$effect` 调后端搜索**无 debounce**。
- 明确"全局"边界：实际为**当前 active context 的 group 快照**（SSH 远程上下文下不含未连接的其他 host），文案/契约不夸大为"所有 host 全局"。

## Capabilities

### New Capabilities
<!-- 无新增 capability，复用 ui-search -->

### Modified Capabilities
- `ui-search`: Command Palette 搜索模式从"已选中项目才显示会话区 / 仅当前组搜索"扩展为"有 query 时全局 sessionId 子串定位（跨所有项目）+ 保留组内正文搜索"；新增去重、排序、最小 query 长度、title best-effort 不触发扫描、跨项目打开归属、active-context 边界等行为契约。

## Impact

- **前端**：`ui/src/components/CommandPalette.svelte`（过滤逻辑、全局遍历、合并去重、stale 快照修复、debounce、UI 行展示项目名/来源）；可能需订阅 `projectDataStore` 响应式快照而非 onMount 一次性复制。
- **spec**：`openspec/specs/ui-search/spec.md`（Command Palette 搜索模式 Requirement + 新 Scenario）。
- **测试**：新增 vitest 单测覆盖全局 id 匹配 / 去重 / 最小长度 / stale 刷新 / 跨项目打开；Playwright user story 覆盖"粘 id 跨项目定位并打开"。
- **不改后端 IPC**（X 阶段零新接口）；不引入文件 I/O；payload 不变（全量 sessionId 已在 `list_repository_groups` 返回内）。
- **无 BREAKING**：纯前端行为增强 + 既有缺陷修复。

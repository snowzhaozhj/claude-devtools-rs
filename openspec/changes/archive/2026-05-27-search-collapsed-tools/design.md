# Design: search-collapsed-tools

## Decisions

### D1: 搜索架构选择——混合搜索（DOM 层 + 数据层虚拟匹配）

**候选方案**：
- A) 全量展开方案：搜索前展开所有 chunk，搜完恢复
- B) 混合搜索：DOM 搜索 + 数据层虚拟匹配，导航时按需展开
- C) 纯数据层搜索：完全抛弃 DOM TreeWalker，改成数据层匹配

**选择 B**。

**理由**：
- A 有性能风险（500+ chunks 全量展开 = 数千 DOM 节点一次性插入）+ 时序复杂（Svelte 5 `{@attach}` 异步）+ 滚动锚点跳变
- C 需要重写整套搜索高亮机制，工作量大且破坏 spec 已有行为契约
- B 保留现有 DOM 搜索完整性 + 零性能风险（数据遍历 < 5ms）+ 按需展开只动单个 chunk

**风险**：虚拟匹配与 DOM 匹配在展开后需要去重，通过重搜机制解决（展开 chunk 后自动 doSearch 更新索引）。

### D2: 虚拟匹配搜索范围——仅 toolName + summary

**候选方案**：
- A) 搜索 toolName + summary + tool output
- B) 搜索 toolName + summary（不含 output）
- C) 仅搜索 toolName

**选择 B**。

**理由**：
- A 需要触发 IPC 懒加载 tool output（outputOmitted 场景），违反 perf 预算
- C 过于狭窄，用户经常搜文件路径（出现在 summary 中，如 Read 的 file_path）
- B 覆盖用户高频搜索场景（工具名 + 文件路径），不触发额外 IPC

### D3: 虚拟匹配导航后去重——展开后重搜

**选择**：导航展开 chunk 后触发一次 doSearch 刷新匹配列表。

**理由**：展开后 DOM 中已有工具名文本，如果不重搜，虚拟匹配和 DOM 匹配会出现重复计数。重搜时已展开 chunk 的工具名会被 DOM 搜索命中，虚拟匹配列表自动排除已展开 chunk → 去重自然完成。

**代价**：导航到虚拟匹配时有一次额外 doSearch（~5ms），可接受。

### D4: SearchBar 接口扩展——新增 virtualMatches prop + onNavigateVirtual 回调

**选择**：SearchBar 新增 `virtualMatches: VirtualMatch[]` prop 和 `onNavigateVirtual: (match: VirtualMatch) => Promise<void>` 回调。

**理由**：
- SearchBar 保持通用性，不直接依赖 SessionDetail 的 chunk 数据结构
- 虚拟匹配的收集逻辑在 SessionDetail 中（它拥有 detail.chunks 和 expandedChunks 状态）
- 导航回调让 SessionDetail 控制展开 + 滚动 + 重搜的时序

## Data Flow

```
用户输入 query
  → SearchBar.doSearch()
    → onBeforeSearch()（hydrate lazy markdown）
    → highlightMatches(container, query) → DOM 匹配 N 个
    → 读取 virtualMatches prop → 虚拟匹配 M 个
    → totalMatches = N + M
    → 显示 "当前 / (N+M)"

用户导航到第 N+k 个匹配（虚拟匹配区域）
  → SearchBar 调 onNavigateVirtual(virtualMatches[k])
  → SessionDetail 展开 chunk + await tick()
  → SessionDetail 定位 [data-tool-use-id] + scrollIntoView
  → SessionDetail 触发 doSearch 重搜（去重）
  → SearchBar 更新 totalMatches + currentIndex
```

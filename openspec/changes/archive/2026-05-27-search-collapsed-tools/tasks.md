# Tasks: search-collapsed-tools

## 1. 实现

- [x] 1.1 SearchBar.svelte：新增 virtualMatches prop + onNavigateVirtual 回调 + 合并计数 + 导航分发
- [x] 1.2 SessionDetail.svelte：收集虚拟匹配（遍历 detail.chunks 的折叠 AI chunk toolExecutions）
- [x] 1.3 SessionDetail.svelte：虚拟匹配导航回调（展开 chunk + tick + 定位 + 重搜）
- [x] 1.4 spec delta：ui-search/spec.md 新增折叠工具名搜索相关 Scenario

## 2. 测试

- [x] 2.1 vitest 单测：虚拟匹配收集逻辑（给定 chunks + expandedChunks + query → 返回正确 VirtualMatch[]）
- [x] 2.2 SearchBar 单测：virtualMatches 合并计数 + 导航分发到 onNavigateVirtual

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

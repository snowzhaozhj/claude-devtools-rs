## ADDED Requirements

### Requirement: Search across all worktrees of a repository group

系统 SHALL 支持按 repository group 维度搜索：给定一组 worktree project ids，收集所有 worktree 对应目录下的 session 文件，全局按 mtime 降序排列后逐文件执行全文搜索。缺失的 worktree 目录 SHALL warn 并跳过，不中断搜索。SSH stage-limit 与 time_budget SHALL 按合并后的总文件数生效。

#### Scenario: Group search merges results from multiple worktrees in mtime order
- **WHEN** group 包含 worktree A（含 session X mtime=100）和 worktree B（含 session Y mtime=200）
- **AND** query 同时命中 session X 和 session Y
- **THEN** 结果 SHALL 含 2 个条目，session Y 在前（mtime 200 > 100）

#### Scenario: Group search skips missing worktree directory
- **WHEN** group 包含 worktree C 但其对应目录不存在
- **AND** worktree A 和 B 正常且命中结果
- **THEN** 结果 SHALL 只含 A 和 B 的命中，不报错

#### Scenario: Group search respects SSH stage-limit across worktrees
- **WHEN** 当前 context 是 SSH 且 group 包含 3 个 worktree 共 200 个 session 文件
- **AND** 搜索在处理第 40 个文件时已命中 min_results 条结果
- **THEN** 搜索 SHALL 在第一阶段上限处提前返回，result.is_partial = true

#### Scenario: Single worktree group degenerates to existing search
- **WHEN** group 仅含 1 个 worktree
- **THEN** 行为 SHALL 等价于对该 worktree 单独调用 `search_sessions`

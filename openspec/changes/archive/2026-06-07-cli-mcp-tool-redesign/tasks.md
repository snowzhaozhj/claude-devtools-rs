## 1. 时间表达式扩展

- [x] 1.1 重构 `parse_duration_to_epoch_ms` 为 `parse_time_expr`，支持相对时长/命名周期/绝对日期三类格式
- [x] 1.2 命名周期（today/yesterday/week）使用 `chrono::Local` 解析本地时区日历边界
- [x] 1.3 绝对日期（`2026-06-06`/ISO 8601）解析，NaiveDate 按本地时区转 epoch
- [x] 1.4 非法格式返回结构化错误（含合法格式示例列表）
- [x] 1.5 MCP `list_sessions` 和 `search_sessions` 暴露 `until` 参数（对接 QueryFilter.until）
- [x] 1.6 CLI `cdt sessions` 添加 `--until` flag
- [x] 1.7 单元测试：覆盖三类格式 + 时区边界 + 非法输入

## 2. list_sessions 增强

- [x] 2.1 新增 `QueryEngine::list_sessions_cross_project(filter)` 方法：遍历 list_repository_groups → 逐 worktree list_sessions_sync → apply filter → merge sort by timestamp desc
- [x] 2.2 跨项目查询添加 mtime 预过滤（stat() 拒绝时间窗口外的 session file）
- [x] 2.3 MCP `list_sessions` handler：`project=None` 时走 cross_project 路径；保持 `project=Some` 时现有路径不变
- [x] 2.4 返回结构添加 `projectName` 字段（从 RepositoryGroup.name 携带）
- [x] 2.5 添加 `branch` 参数：对 head-read metadata 中的 gitBranch 做 case-insensitive substring 匹配
- [x] 2.6 添加 `group_by` 参数（none/project/day）：在返回信封中增加 `groups` 字段
- [x] 2.7 添加 `is_ongoing` 参数：布尔过滤活跃 session
- [x] 2.8 CLI `cdt sessions` 同步添加 `--branch`/`--group-by`/`--is-ongoing` flags
- [x] 2.9 测试：跨项目查询 + branch 过滤 + group_by + mtime 预过滤

## 3. get_session 复合工具

- [x] 3.1 新增 `QueryEngine::inspect_session(session_id, project, include)` 方法：一次 get_session_detail → 按需 build_summary/compute_cost/extract_errors
- [x] 3.2 定义 `InspectResult` 返回结构：紧凑默认（metadata + cost + errorCount + 前 10 条 error）+ 可选 facets（phases/tools/activity/idle_gaps/files）
- [x] 3.3 数据完整性：errorMessage 超长时用 head+tail 摘要 + `messageSummarized: true`（阈值由 errorMessage 长度分布分析确定）
- [x] 3.4 session ID 支持 'latest' 别名：`resolve_session_id` 检测 literal "latest" → 返回最近一条
- [x] 3.5 MCP `get_session` tool handler + `GetSessionParams` struct（session/project/include/exclude）
- [x] 3.6 CLI `cdt session <id> [--include] [--format]` 子命令
- [x] 3.7 废弃别名：get_session_summary/get_session_cost/get_session_errors 转发到 get_session
- [x] 3.8 测试：复合返回完整性 + include 各组合 + latest 解析 + 废弃别名转发

## 4. get_session_chunks（改名 + 增强）

- [x] 4.1 MCP tool 名从 `get_session_detail` 改为 `get_session_chunks`
- [x] 4.2 保留 `grep`/`grep_context` 参数（方案 A 最终决策：发现 vs 过滤检索是不同意图，不迁移）
- [x] 4.3 新增 `content_mode="overview"` 模式：每 chunk 返回 chunkIndex/kind/timestamp/toolNames/errorCount/headline（首 100 字符语义摘要）
- [x] 4.4 CLI 命令改为 `cdt session <id> --chunks [--range] [--tail] [--filter] [--content]`
- [x] 4.5 测试：overview 模式输出 + grep 移除后无回归

## 5. search_sessions 增强

- [x] 5.1 MCP `search_sessions` 添加 `since` 参数：先按 mtime 过滤 session 列表再执行搜索
- [x] 5.2 CLI `cdt search` 添加 `--since` flag
- [x] 5.3 测试：since 预过滤减少 sessionsSearched + intra-session search（session 参数）行为验证

## 6. get_stats 实现

- [x] 6.1 新增 `cdt-parse` 浅解析模式 `parse_session_shallow`：逐行提取 TokenUsage + tool_name + is_error 字段，不构建 chunk 树
- [x] 6.2 新增 `QueryEngine::aggregate_stats(filter, group_by)` 方法：流式聚合（解析一个 session → 贡献到聚合结果 → 释放 → 下一个）
- [x] 6.3 MCP `get_stats` handler 替换现有 stub（返回 AggregatedStats + group_by 分组）
- [x] 6.4 添加 `group_by` 参数（none/project/model/day）
- [x] 6.5 CLI `cdt stats` 添加 `--group-by` flag
- [x] 6.6 测试：浅解析 vs 全量解析结果对比 + group_by 分组 + 流式聚合内存安全

## 7. CLI 自动补全更新

- [x] 7.1 更新 `completions.rs`：新增 `cdt session` 位置参数的 SessionCompleter
- [x] 7.2 `--since`/`--until` 值补全：today/yesterday/7d/24h/30d
- [x] 7.3 `--group-by` 值补全：none/project/day（sessions）/ none/project/model/day（stats）
- [x] 7.4 `--include` 值补全：phases/tools/activity/idle_gaps/files
- [x] 7.5 `--content` 值补全：compact/overview/full
- [x] 7.6 `--filter` 值补全：errors_only/tool_calls
- [x] 7.7 验证 bash/zsh/fish 补全脚本生成无报错

## 8. MCP server instructions 更新

- [x] 8.1 重写 `get_info().instructions`：决策树式引导（按意图选工具 + 负向指引）
- [x] 8.2 每个 tool description 加入 "when to use" / "when NOT to use" 引导
- [x] 8.3 验证 MCP server 启动 + tool list 返回新工具集

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过
- [ ] N.4 archive change

## Why

当前 CLI/MCP 工具面从数据实体角度设计（8 个工具对应 projects/sessions/summary/cost/errors/detail/search/stats），导致 agent 回答一个简单问题需要大量串行调用——实测"分析昨天所有会话"需要 34 次调用（list_projects → 逐项目 list_sessions → 逐会话 get_session_summary/cost）。每次调用的后端执行仅 50ms，但 AI 推理一轮 3-8 秒，真正瓶颈是调用次数而非后端性能。

需要从面向实体的 CRUD 接口重设计为面向用户意图的查询接口，让常见场景 ≤2 次调用即可回答。

## What Changes

- **list_sessions 增强**：`project` 参数改为可选（省略=全局跨项目查询）；`since`/`until` 支持绝对日期（ISO 8601）和命名周期（'yesterday'/'today'）；新增 `branch`/`group_by`/`is_ongoing` 参数
- **新建 get_session 复合工具**：合并 get_session_summary + get_session_cost + get_session_errors 为单次调用，默认返回紧凑复合视图（指标 + cost + 前 N 条 errors），`include` 参数按需追加重数据（phases/tools/activity/idle_gaps）
- **get_session_detail → get_session_chunks 改名**：移除 grep/grep_context 参数（移至 search_sessions），新增 `content_mode='overview'`
- **search_sessions 增强**：新增 `since` 时间预过滤参数；接管 session 内 grep 功能（session 参数 + query）
- **get_stats 真实实现**：从 MCP stub 变为完整实现，新增 `group_by` 参数（project/model/day）
- **删除 3 个工具**（过渡期保留为废弃别名 4-6 周后移除）：get_session_summary / get_session_cost / get_session_errors **BREAKING**
- **数据完整性规则**：每条记录完整返回不切字段；超长文本用 head+tail 摘要 + `messageSummarized` 标记
- **session ID 支持 'latest' 别名**：自动解析为最近一次会话

## Capabilities

### New Capabilities

（无新 capability——本次是对现有 capability 的接口层重设计）

### Modified Capabilities

- `mcp-server`：工具集从 8 个重组为 6 个；新增 get_session 复合工具；list_sessions 参数扩展（project 可选、since/until 格式、branch/group_by/is_ongoing）；search_sessions 加 since；get_stats 真实实现；get_session_detail 改名 get_session_chunks + 移除 grep；数据完整性规则（head+tail 摘要）
- `cli-output`：CLI 命令结构变化（`cdt session <id>` 新命令、`cdt sessions` 增强、`cdt session <id> --chunks` 替代 `cdt sessions detail`）；时间表达式解析扩展

## Impact

- **crates/cdt-cli/src/mcp/mod.rs**：MCP 工具定义重构（新增 get_session、改造 list_sessions/search_sessions/get_stats、废弃 3 个旧工具）
- **crates/cdt-cli/src/main.rs**：CLI 命令结构调整（新增 `cdt session` 子命令、`cdt sessions` 参数扩展）
- **crates/cdt-query/src/engine.rs**：新增 `list_sessions_cross_project`、`inspect_session`（get_session 后端）、`aggregate_stats` 方法
- **crates/cdt-query/src/filter.rs**：`parse_duration_to_epoch_ms` 扩展支持绝对日期/命名周期/本地时区
- **crates/cdt-cli/src/view.rs**：新增 `ContentMode::Overview` 模式
- **openspec/specs/mcp-server/spec.md**：工具集 scenario 全量更新
- **openspec/specs/cli-output/spec.md**：命令结构 scenario 更新

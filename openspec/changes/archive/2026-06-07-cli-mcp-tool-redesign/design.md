## Context

现有 MCP server 暴露 8 个 tools（`list_projects`/`list_sessions`/`get_session_summary`/`get_session_cost`/`get_session_errors`/`get_session_detail`/`search_sessions`/`get_stats`），设计来源于 CLI 子命令的 1:1 映射，面向数据实体。实测 agent 回答"分析昨天所有会话"需要 34 次调用，瓶颈不在后端性能（每次 <100ms），在 AI 推理 round-trip（每次 3-8s）。

代码位置：
- MCP 工具定义：`crates/cdt-cli/src/mcp/mod.rs`（~888 行）
- CLI 命令定义：`crates/cdt-cli/src/main.rs`（~1960 行）
- QueryEngine：`crates/cdt-query/src/engine.rs`
- QueryFilter：`crates/cdt-query/src/filter.rs`（`until` 字段已实现但未暴露）
- 时间解析：`parse_duration_to_epoch_ms` 在 `mcp/mod.rs`

约束：
- CLI 和 MCP 共享 `Arc<QueryEngine>`，操作语义 MUST 统一
- HTTP API（60+ 路由）服务桌面端，不参与本次重设计
- 无外部用户，不需要兼容性保证
- 工具数量控制在 6-12 个（schema token 开销考量）

## Goals / Non-Goals

**Goals:**
- 常见 agent 场景 ≤2 次调用可回答（现 34 次 → 1 次）
- CLI 和 MCP 完全统一（同操作同参数同语义）
- 每条记录完整返回不切字段（数据完整性）
- 扩展性：新能力通过参数添加，8 参数以内不加新工具

**Non-Goals:**
- 不改 HTTP API（桌面端接口独立演进）
- 不建索引/数据库（当前 JSONL-on-disk 模型对 <500 sessions 足够）
- 不做实时 streaming/订阅（MCP streaming 生态未成熟）
- 不做写操作（tag/archive/metadata update）

## Decisions

### D1：list_sessions project 参数改为可选

**决策**：`project` 从必需改为可选；省略时全局扫描所有项目。

**候选方案**：
- A：保持 project 必需 + 新增独立的 `list_all_sessions` 工具
- B：project 改可选，QueryEngine 内部区分两条路径

**选择 B**，理由：
- 不增加工具数量（原则 3：参数扩展非工具膨胀）
- "跨项目 + 单项目"是同一操作的两种 scope，不是两种 intent
- 内部实现：`project=None` 时调 `QueryEngine::list_sessions_cross_project(filter)`，遍历 `list_repository_groups()` → 逐 worktree `list_sessions_sync` → apply filter → merge + sort by timestamp desc

**风险**：大 corpus（30 project × 1500 session）全局查询延迟。

**缓解（codex D1 修订）**：
1. 跨项目路径不复用现有 `list_sessions_sync(page_size=usize::MAX)`，新增独立方法在 scanner 层按 mtime 预过滤（stat() 拒绝时间窗口外的 session file，**在**读 metadata 之前）
2. `since` 在跨项目模式下 SHALL 有强制默认值 `'7d'`（MCP ListSessionsParams 中 since=None + project=None 时自动填充 '7d'）
3. limit 默认 20，跨项目排序在 filter 后 take(limit) 提前截断，不全量 sort

### D2：合并 get_session_summary + get_session_cost + get_session_errors → get_session

**决策**：新建 `get_session` 复合工具，一次调用返回 summary + cost + errors。旧三个工具保留为废弃别名。

**候选方案**：
- A：保留三个独立工具 + 教 agent 并行调用
- B：合并为一个复合工具 + include 参数控制重数据

**选择 B**，理由：
- 三个工具共享同一 JSONL 解析路径，分开调 = 同一文件解析 3 次
- Agent 的 MCP instructions 已经建议"先调 summary 再按需 detail"——承认了分开的不便
- `build_summary` 内部已调用 `compute_session_cost`，cost 是 summary 的副产物
- 默认返回紧凑视图（指标 + cost + 前 10 条 error），`include` 追加 phases/tools/activity/idle_gaps

**实现（codex D2 修订）**：`QueryEngine::inspect_session(session_id, project, include)` → 调一次 `get_session_detail` → 当 include 含 summary 或 cost 时调 `build_summary`（内部已含 cost 计算），从 `summary.cost` 提取 cost 字段（不再单独调 `compute_session_cost`，避免重复计算）→ extract_errors → 组合返回。`include` 参数解析为逗号分隔枚举集（`phases`/`tools`/`activity`/`idle_gaps`/`files`），未知值 SHALL 返回 `invalid_params` 错误。

### D3：时间表达式扩展

**决策**：`since`/`until` 参数统一支持三类格式：
1. 相对时长：`'7d'`/`'24h'`/`'1h'`/`'30m'`（现有）
2. 命名周期：`'today'`/`'yesterday'`/`'week'`（新增）
3. 绝对日期：`'2026-06-06'`/ISO 8601（新增）

**候选方案**：
- A：只加 `until`，不扩展 `since` 格式
- B：全面扩展 + 本地时区处理

**选择 B**，理由：
- Agent 最常表达的时间概念是 "yesterday"/"today"，不是 "24h"
- `yesterday` 需要本地时区解析（chrono::Local），不是 UTC-24h
- Duration（'24h'）保持 UTC 相对，Calendar（'yesterday'）用 Local

**实现（codex D3 修订）**：新建 `parse_time_expr(expr: &str, now: DateTime<Utc>, local_tz: &impl TimeZone) -> Result<i64>`，注入 `now` 和 timezone provider，使单元测试可固定 `now=2026-06-07T02:00:00Z` + `tz=Asia/Shanghai` 而不依赖运行机环境。生产调用传 `Utc::now()` + `chrono::Local`。CI 环境（TZ 通常为 UTC）下 'yesterday' 测试仍确定性——因为 now 和 tz 都是注入的。

### D4：get_session_detail 改名 get_session_chunks + grep 移出

**决策**：
- 改名为 `get_session_chunks`（CLI: `cdt session <id> --chunks`）
- `grep`/`grep_context` 移出到 `search_sessions(session=X, query=Y)`
- 新增 `content_mode='overview'`（每 chunk 一行结构摘要）

**候选方案**：
- A：保留 grep 在 detail 里（不改）
- B：grep 移到 search_sessions（session 内搜索统一用 search）

**最终决策：方案 A——grep 保留在 get_session_chunks（不迁移）**

经 codex 二轮评估，三个方案对比如下：
- 方案 B（统一 chunk 级返回）：跨 session 搜索 payload 爆炸（100 hits × ChunkView = 500KB 吃光 agent context）——**致命缺陷，淘汰**
- 方案 C（双模返回）：同一工具条件性返回不同结构是 MCP 反模式，增加 agent 强制认知负担——**淘汰**
- 方案 A（保留 grep）：零行为风险 + 正确语义分层——**胜出**

**关键认知**：`search_sessions` 和 `get_session_chunks(grep=X)` 服务的是**两个不同意图**：
- search_sessions = **发现**："哪些 session 提到了 X？" → 轻量 snippets 帮 agent 定位
- get_session_chunks + grep = **过滤检索**："我已经在看 session S，给我匹配 X 的 chunks 及上下文" → 完整 ChunkView envelope

这不是"搜索语义分裂"，是正确的意图分层。两个工具各自沿自己的职责演进。

**参数数量**：get_session_chunks 10 个参数（session/project/range/tail/filter/content_mode/max_chunks/cursor/grep/grep_context），其中 required 只有 session（1 个），grep/grep_context 是条件依赖可选。实际 MCP 生态中 10+ 可选参数的工具正常运作（GitHub create_pull_request、Notion search 等）。8 参数指导线针对的是 required 太多的问题，本场景不适用。

### D5：get_stats 实现策略——浅解析模式

**决策**：`get_stats` MCP 实现使用浅解析模式（只提取 TokenUsage 字段），不做全量 chunk build。

**候选方案**：
- A：复用现有 `get_session_detail` + `build_summary` 全量路径
- B：新增浅解析模式，只读 JSONL 中的 usage 和 tool_name/is_error 字段

**选择 B**，理由：
- 全量路径每 session 60-150ms，100 session = 6-15s，agent 等太久
- stats 只需 token 数 + 工具名 + 是否 error，无需完整 chunk 树
- 浅解析跳过 tool-linking/context-annotation/workflow-manifest，约 10-20% full cost

**实现（codex D5 修订）**：`cdt-parse` 新增 `parse_session_shallow` 函数，复用现有 `parse_entry_at` 的 `Raw` 结构体做逐行 serde 反序列化（不用独立 regex——避免与 full parser 口径分叉），只提取 `message.usage`（TokenUsage）和 `content` blocks 中的 `ToolUse.name` / `ToolResult.is_error` 字段，跳过 tool-linking/chunk-building/context-annotation。验收测试 SHALL 对比 shallow vs full 的 cost、tool frequency、error count 结果一致性（覆盖连续 assistant 多 usage、tool_use/tool_result 分离场景）。

### D6：数据完整性——head+tail 摘要策略

**决策**：超长文本字段（errorMessage、tool output）不做硬截断（不丢数据），改用 head + "…" + tail 保留首尾。

**候选方案**：
- A：固定 500 字符截断 + `messageTruncated: true`（现状）
- B：head + tail 摘要 + `messageSummarized: true`

**选择 B**，理由：
- Root cause 通常在 stack trace 尾部（最后几行），截前 500 字符恰好丢掉关键信息
- head+tail 保留首尾上下文，agent 据此可判断是否需要深入
- 具体阈值（head 多少、tail 多少、总体积上限）由实现阶段分析 errorMessage 长度分布后确定

**标记**：字段名为 `messageSummarized: true`（语义是"被摘要了"非"被截断了"），agent 通过 `chunkIndex` 定位全文。

**适用范围（codex D6 修订）**：head+tail 摘要**仅对 string 类型字段**生效（errorMessage 等纯文本）。结构化 JSON Value（如 tool output 的 `serde_json::Value`）SHALL NOT 做内部截断（会破坏 JSON 结构），改为整体省略 + 返回 `outputOmitted: true` + `outputChars: N`（现有行为，保持不变）。阈值单位为 chars（char boundary），实现使用 `.chars()` 迭代确保 UTF-8 安全。

### D7：session ID 支持 'latest' 别名

**决策**：所有接受 session ID 的工具支持保留字 `'latest'`，解析为当前 scope 内最近的会话。

**实现（codex D7 修订）**：`resolve_session_id` 中检测 literal `"latest"`：
- `project=None` → 全 corpus 按 session file mtime 降序取第一条（全局最新）
- `project=Some(p)` → 该 project 范围内按 mtime 降序取第一条
两种情况复用同一排序函数（`sort_sessions_by_mtime_desc`），避免 group 排序和 session 排序产生差异。

## Performance Impact

本次改动不引入性能回归，D2（复合 get_session）严格更优（少 2 次 JSONL 解析）。新增能力的 perf 预算：

| 路径 | wall 预算 | 场景 | 机制 |
|---|---|---|---|
| list_sessions 跨项目（since='7d'，~30 sessions） | < 300ms | 默认调用 | mtime 预过滤 + limit=20 截断；stat() 用 `tokio::fs::metadata` 不阻塞 worker |
| list_sessions 跨项目（since='30d'，~200 sessions） | < 500ms | 最坏常见 | 同上，mtime 淘汰大部分后 metadata read ~50 条 |
| get_session 复合（单 session） | < 150ms | 等同现 get_session_summary | 1 次 JSONL parse（现状 3 次），FileSignature cache 命中后近瞬时 |
| get_stats 7d（~30 sessions，shallow） | < 500ms | 典型调用 | 浅解析 ~10-15ms/session；已完成 session cache 命中近瞬时 |
| get_stats 30d（~100 sessions，cold） | < 1.5s | 最坏情况 | 浅解析 cold 上限；repeated call 走 cache |

**反模式合规**：
- D1 跨项目 stat() 循环 SHALL 用 `tokio::fs::metadata`（async），不阻塞 tokio worker
- 无新 subprocess spawn / 无 O(N²) / 无 IPC payload > 1MB
- get_session 默认响应 ~1.5KB，list_sessions limit=20 不超标

## Risks / Trade-offs

- **[跨项目查询延迟]** → mtime 预过滤 + since 强制默认 '7d' + limit=20 约束最坏情况；超预算时 log warning
- **[get_stats 浅解析准确性]** → 复用 Raw 结构体不用独立 regex；验收测试对比 shallow vs full（20 session 样本）
- **[废弃别名 token 开销]** → 过渡期 4-6 周后移除；过渡期每 session 多 ~1500 token schema 开销可接受
- **[get_session_detail → get_session_chunks 改名]** → 现有 skill/instructions/测试中引用 `get_session_detail` 的地方需要同步更新；grep/grep_context 保留不迁移
- **[进行中会话一致性]** → FileSignature cache 在 mtime 变化时自动失效；parser 容忍截断末行

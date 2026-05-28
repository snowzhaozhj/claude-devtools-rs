# Design: mcp-server-stdio

## Decisions

### D1: MCP Server 放在 cdt-cli 内，不单拆 crate

**选**：`cdt-cli/src/mcp/` 模块，作为 `cdt mcp serve` 子命令的实现。

**弃选**：新建 `cdt-mcp` library crate。

**理由**：Phase 1 只有 CLI binary 消费 MCP 逻辑；桌面端内嵌是 future 需求。模块与 binary 同生命周期最简单。如果后续 `src-tauri` 需要内嵌 MCP，把 `src/mcp/` 整目录移到独立 crate 成本 ~30 min（接口 `CdtMcpServer::new(engine)` 不变）。

### D2: summary/cost/stats 从 cdt-cli 下沉到 cdt-query

**选**：将 `cdt-cli/src/summary.rs`、`cost.rs`、`stats.rs` 中的纯算法逻辑移到 `cdt-query`。CLI 和 MCP 是 cdt-query 的对称消费者。cdt-cli 只保留 format/display 逻辑。

**弃选**：MCP 在 cdt-cli 内部 `use crate::summary` 直接调。

**理由**：summary/cost/stats 是纯函数（输入 `Vec<Chunk>`/metadata，输出结构化数据），属于 query 层职责。CLI formatter 和 MCP server 是平级消费者，不应互相依赖。下沉后 HTTP serve 模式也能复用。

### D3: 8 个粗粒度 Tools，不细拆

**选**：8 个 tools 与 CLI 子命令 1:1 对应：`list_projects`、`list_sessions`、`get_session_summary`、`get_session_detail`、`get_session_errors`、`search_sessions`、`get_session_cost`、`get_stats`。

**弃选**：12+ 细粒度 tools（拆 timeline / tool_usage / daily_stats 等）。

**理由**：AI 选择越少决策越快；`get_session_summary` 一次返回完整诊断比分三次调高效得多；optional 参数足够表达过滤需求。

### D4: TokenEstimator trait 预留扩展，默认粗估

**选**：定义 `pub trait TokenEstimator: Send + Sync { fn estimate(&self, text: &str) -> usize; }`，默认实现 `CharRatioEstimator`（~4 chars/token）。trait 放 cdt-query。

**弃选**：硬编码 `text.len() / 4`。

**理由**：后续可换 tiktoken/cl100k 只需加实现；trait object 虚调用开销在 MCP 场景不可见（每次 tool call 最多调一次）。

### D5: Redaction 在 MCP 层，不在 QueryEngine

**选**：MCP server 层对 `CallToolResult` 内容做正则脱敏（API key / token / password patterns → `[REDACTED]`）。CLI 不 redact。`--allow-sensitive` flag 绕过。

**弃选**：QueryEngine 层统一 redact。

**理由**：CLI 用户查自己数据不需要脱敏；MCP 返回进 AI context 可能被无意暴露。层次分离：query 只管查，MCP 只管安全包装。

### D6: 结构化截断——按 chunk 粒度，不切半条

**选**：`truncate_to_budget(chunks, estimator, budget)` 按整 chunk 截断。超限时返回 `{ "chunks": [...], "truncated": true, "totalChunks": N, "nextRange": "51:" }`。

**弃选**：字符级截断 / 不截断。

**理由**：切半条 JSON 会破坏结构；`nextRange` 让 AI 知道可以继续翻页；结构化截断保证每次返回都是合法 JSON。

### D7: 注册写入全局 `~/.claude/mcp_servers.json`

**选**：`cdt setup mcp --apply` 写入全局配置。不带 `--apply` 时打印配置 + diff 预览。写入保留已有未知字段（原子写入：tmp → rename）。

**弃选**：写入项目级 `.mcp.json`。

**理由**：session intelligence 是跨项目能力（分析任意项目的 session），属于全局工具；项目级注册需要每个项目都配一次。

### D8: 不做 session parsed cache

**选**：每次 tool call 全量解析 session（当前实测 1221 msg → 60-74ms）。

**弃选**：cdt-query 层加 LRU cache。

**理由**：MCP stdio 是单客户端、秒级间隔（AI 推理 3-15s），70ms × 3 = 210ms 分散在 10+ 秒完全不可见。cache 的失效复杂度（ongoing 追加 / compaction / 多文件 / mtime 精度）远超收益。真正的大 session 优化方向是 range-based lazy parsing（跳过前 N 行不解析），不是缓存。经 codex 二审确认降级为 LOW。

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        cdt-cli (binary)                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  main.rs          clap dispatch: mcp serve → run_mcp_server()       │
│  mcp/mod.rs       CdtMcpServer struct + rmcp #[tool_router]         │
│  mcp/redact.rs    Redactor（正则脱敏）                               │
│  mcp/truncate.rs  truncate_to_budget（结构化截断）                   │
│                                                                     │
└──────────────┬──────────────────────────────────────────────────────┘
               │ depends on
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        cdt-query (library)                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  engine.rs        QueryEngine                                       │
│  summary.rs       SessionSummaryOutput（从 cdt-cli 下沉）            │
│  cost.rs          SessionCost + per-model pricing（下沉）            │
│  stats.rs         AggregatedStats（下沉）                            │
│  token.rs         TokenEstimator trait + CharRatioEstimator          │
│  filter.rs        QueryFilter                                       │
│  options.rs       SessionQueryOptions                                │
│                                                                     │
└──────────────┬──────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│            cdt-api / cdt-core / cdt-parse / cdt-analyze              │
└─────────────────────────────────────────────────────────────────────┘
```

## MCP Server initialization flow

```
cdt mcp serve
    │
    ├── tracing → stderr（stdout 是 JSON-RPC 通道）
    ├── build_local_data_api()（无 watcher 模式）
    ├── QueryEngine::new(api)
    ├── CdtMcpServer::new(engine, redactor, estimator)
    │
    ├── server.serve(rmcp::transport::stdio()).await
    └── service.waiting().await
```

## Tool schema overview

| Tool | 参数 | 返回 |
|------|------|------|
| `list_projects` | — | `[{name, path, sessions, lastActive}]` |
| `list_sessions` | `project?`, `since?`, `grep?`, `limit?` | `[{id, title, duration, status, messages}]` |
| `get_session_summary` | `session`, `project?` | `{phases, toolUsage, errors, idleGaps, cost, ...}` |
| `get_session_detail` | `session`, `project?`, `range?`, `tail?`, `filter?`, `max_tokens?` | `{chunks: [...], truncated?, totalChunks?, nextRange?}` |
| `get_session_errors` | `session`, `project?` | `[{chunkIndex, toolName, errorMessage}]` |
| `search_sessions` | `query`, `limit?`, `project?` | `[{sessionId, title, matches}]` |
| `get_session_cost` | `session`, `project?` | `{totalInput, totalOutput, byModel, estimatedCost}` |
| `get_stats` | `period?`, `project?` | `{sessions, messages, tools, models, activeHours}` |

所有 tools 加 annotations：`read_only_hint=true`, `destructive_hint=false`, `idempotent_hint=true`, `open_world_hint=false`。

`project` 参数在除 `list_projects` 外均可选——省略时由 `find_session_project(session_id)` 自动解析。

## Concurrency model

- rmcp spawn 独立 tokio task 处理每个 JSON-RPC request → 并发 tool call 是现实情况
- `Arc<QueryEngine>` 内部 `Arc<LocalDataApi>` 已是 Send + Sync
- CPU 重活（大 session 解析）走 `tokio::task::spawn_blocking` 避免卡 async worker
- MCP cancellation：tool handler 接收 `RequestContext` 的 cancellation token，长查询定期检查

## Redaction patterns

```
sk-[a-zA-Z0-9_-]{20,}              # Anthropic / OpenAI API keys
AKIA[A-Z0-9]{16}                   # AWS access key
ghp_[a-zA-Z0-9]{36,}              # GitHub PAT
Bearer [a-zA-Z0-9._-]{20,}        # Bearer tokens
password\s*[=:]\s*\S+             # password assignments
-----BEGIN .* PRIVATE KEY-----    # private key headers
eyJ[a-zA-Z0-9_-]{20,}            # JWT tokens (base64 header)
```

返回体附加 `"redacted": true` + `"redactedCount": N` 元信息。

## Risks

- **rmcp 版本稳定性**：v1.7 两周前发布，API 可能小幅变动。缓解：锁精确版本 `=1.7.0`，不用 `^`。
- **schemars 兼容性**：rmcp 用 `schemars 1.0`，workspace 可能用旧版。缓解：检查 workspace deps，必要时升级。
- **stdout 污染**：任何意外 println! / panic backtrace 写入 stdout 会破坏 JSON-RPC 流。缓解：`#[cfg(test)]` 之外禁止 `println!`；panic hook 写 stderr。

## Changes by file

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | 新增 `rmcp`, `schemars` workspace deps |
| `crates/cdt-query/Cargo.toml` | 新增 `schemars` dep |
| `crates/cdt-query/src/lib.rs` | 新增 `pub mod summary`, `cost`, `stats`, `token` |
| `crates/cdt-query/src/summary.rs` | 从 cdt-cli 迁入（改 pub 可见性） |
| `crates/cdt-query/src/cost.rs` | 从 cdt-cli 迁入 |
| `crates/cdt-query/src/stats.rs` | 从 cdt-cli 迁入 |
| `crates/cdt-query/src/token.rs` | 新建：`TokenEstimator` trait + `CharRatioEstimator` |
| `crates/cdt-cli/Cargo.toml` | 新增 `rmcp` dep |
| `crates/cdt-cli/src/mcp/mod.rs` | 新建：`CdtMcpServer` + `#[tool_router]` |
| `crates/cdt-cli/src/mcp/redact.rs` | 新建：`Redactor` 正则脱敏 |
| `crates/cdt-cli/src/mcp/truncate.rs` | 新建：`truncate_to_budget` |
| `crates/cdt-cli/src/main.rs` | `McpAction::Serve` dispatch → `run_mcp_server()` |
| `crates/cdt-cli/src/summary.rs` | 改为 thin wrapper 调 `cdt_query::summary` |
| `crates/cdt-cli/src/cost.rs` | 改为 thin wrapper |
| `crates/cdt-cli/src/stats.rs` | 改为 thin wrapper |

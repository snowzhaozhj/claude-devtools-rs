## Why

claude-devtools-rs 拥有完整的 session 解析/分析/搜索能力（cdt-parse → cdt-analyze → cdt-query），但这些能力只能通过 CLI 手动调用或 HTTP API 访问。Claude Code 等 AI IDE 无法自动发现和调用这些能力来分析 session 历史。

MCP（Model Context Protocol）是 AI IDE 的标准工具协议——通过 stdio transport 暴露 tools，让 AI 自主查询 session 数据进行诊断、趋势分析、成本统计。

## What Changes

为 `cdt mcp serve` 子命令实现 MCP stdio server：

1. 使用 rmcp v1.7（官方 Rust MCP SDK）+ stdio transport
2. 暴露 8 个 read-only tools，与 CLI 子命令 1:1 对应
3. 分层上下文控制：summary（~2K token）→ detail 按需深入（range/tail/max_tokens）
4. MCP 层自动 redaction（脱敏 secret patterns）
5. 结构化截断（按 chunk 粒度，不切半条 JSON）

**新增 capability**：`mcp-server`（新 spec）

**Crates 改动**：
- `cdt-query`：下沉 summary/cost/stats 模块 + 新增 `TokenEstimator` trait
- `cdt-cli`：新增 `src/mcp/` 模块（MCP server 实现）+ 更新 `cdt mcp serve` dispatch

**依赖**：新增 `rmcp`（features: server, macros, transport-io, schemars）+ `schemars`

**注册**：`cdt setup mcp --apply` 写入 `~/.claude/mcp_servers.json`

## Non-goals

- HTTP/SSE MCP transport（Phase 2）
- MCP Resources（Phase 1 只用 Tools）
- Session parsed cache（不需要——70ms 解析在 MCP round-trip 中不可见）
- 审计日志（Phase 1 不做）
- 桌面端内嵌 MCP（未来需求再拆 crate）
- 进度通知（`notifications/progress`）

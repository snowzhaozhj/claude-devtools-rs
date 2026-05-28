# Tasks: mcp-server-stdio

## 1. 下沉 summary/cost/stats 到 cdt-query

- [x] 1.1 在 cdt-query 新建 `src/token.rs`：`TokenEstimator` trait + `CharRatioEstimator` 实现
- [x] 1.2 将 `cdt-cli/src/summary.rs` 核心逻辑迁移到 `cdt-query/src/summary.rs`（保留 pub types + `build_summary` 函数）
- [x] 1.3 将 `cdt-cli/src/cost.rs` 核心逻辑迁移到 `cdt-query/src/cost.rs`
- [x] 1.4 将 `cdt-cli/src/stats.rs` 核心逻辑迁移到 `cdt-query/src/stats.rs`
- [x] 1.5 cdt-cli 的 summary/cost/stats 改为 thin wrapper（调 cdt-query + 格式化输出）
- [x] 1.6 确保 `cargo test -p cdt-query` 和 `cargo test -p cdt-cli` 全绿

## 2. MCP Server 核心实现

- [x] 2.1 workspace `Cargo.toml` 新增 `rmcp = { version = "1.7", features = ["server", "macros", "transport-io"] }` + `schemars = "1.0"`
- [x] 2.2 `cdt-cli/Cargo.toml` 新增 `rmcp` + `schemars` 依赖
- [x] 2.3 新建 `cdt-cli/src/mcp/mod.rs`：`CdtMcpServer` struct 持有 `Arc<QueryEngine>` + `Redactor` + `Arc<dyn TokenEstimator>`
- [x] 2.4 实现 `#[tool_router(server_handler)]` 注册 8 个 tools：
  - [x] `list_projects`
  - [x] `list_sessions`
  - [x] `get_session_summary`
  - [x] `get_session_detail`
  - [x] `get_session_errors`
  - [x] `search_sessions`
  - [x] `get_session_cost`
  - [x] `get_stats`
- [x] 2.5 实现 `get_info()` 返回 `ServerInfo`（含 instructions 引导 AI 使用模式）
- [x] 2.6 `main.rs` 的 `McpAction::Serve` 分支：构造 QueryEngine → CdtMcpServer → `serve(stdio())` → `waiting()`
- [x] 2.7 tracing 确保只写 stderr + panic hook 写 stderr

## 3. Redaction 模块

- [x] 3.1 新建 `cdt-cli/src/mcp/redact.rs`：`Redactor` struct + 正则 pattern 集合
- [x] 3.2 实现 `redact(&self, text: &str) -> (String, usize)` 返回脱敏文本 + redacted count
- [x] 3.3 `--allow-sensitive` flag 传入 `CdtMcpServer` 构造时控制是否启用 redaction
- [x] 3.4 单元测试：各种 secret pattern 被正确替换

## 4. 结构化截断

- [x] 4.1 新建 `cdt-cli/src/mcp/truncate.rs`：`truncate_to_budget(chunks, estimator, budget) -> TruncatedResult`
- [x] 4.2 `TruncatedResult` 包含 `chunks`、`truncated: bool`、`total_chunks: usize`、`next_range: Option<String>`
- [x] 4.3 `get_session_detail` tool 在返回前调用截断
- [x] 4.4 单元测试：验证 chunk 粒度截断 + next_range 正确

## 5. Setup 命令

- [x] 5.1 实现 `cdt setup mcp`（不带 --apply）：打印 JSON 配置片段
- [x] 5.2 实现 `cdt setup mcp --apply`：读取现有 `~/.claude/mcp_servers.json` → merge `cdt` entry → 原子写入（tmp + rename）+ 打印 diff
- [x] 5.3 保留已有未知字段（serde_json::Value round-trip）

## 6. 测试与验证

- [x] 6.1 集成测试：MCP server 启动 → 通过 duplex transport 发 tool call → 验证 JSON 返回结构
- [x] 6.2 Redaction 测试：含 API key 的 session 返回中 key 被替换
- [x] 6.3 截断测试：超 budget 时返回 truncated=true + nextRange
- [x] 6.4 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [ ] 6.5 手动验证：`cdt mcp serve` 启动后用 MCP Inspector 或 Claude Code 连接成功

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

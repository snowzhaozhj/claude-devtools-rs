## 1. 依赖 + 脚手架

- [x] 1.1 在 `cdt-analyze/Cargo.toml` 添加 `regex` workspace 依赖
- [x] 1.2 建立 `cdt-analyze/src/team/` module 结构：`mod.rs`、`detection.rs`、`summary.rs`、`enrichment.rs`
- [x] 1.3 `cargo build -p cdt-analyze` 确认编译通过

## 2. Teammate 消息检测

- [x] 2.1 在 `detection.rs` 实现 `is_teammate_message(msg: &ParsedMessage) -> bool`：检测 `<teammate-message teammate_id="...">` 标签
- [x] 2.2 实现 `TeammateAttrs { teammate_id, color, summary, body }` 和 `parse_teammate_attrs(msg: &ParsedMessage) -> Option<TeammateAttrs>`
- [x] 2.3 单元测试：string content 匹配、block content 匹配、非 teammate 消息、attribute 提取

## 3. Team 工具摘要

- [x] 3.1 在 `summary.rs` 实现 `is_team_tool(name: &str) -> bool`（`TeamCreate`/`TaskCreate`/`TaskUpdate`/`TaskList`/`TaskGet`/`SendMessage`/`TeamDelete`）
- [x] 3.2 实现 `format_team_tool_summary(name: &str, input: &serde_json::Value) -> String`：按工具分发到专用格式
- [x] 3.3 单元测试：每种 tool 的摘要格式、`SendMessage` shutdown_response 场景

## 4. Team 元数据富化

- [x] 4.1 在 `enrichment.rs` 实现 `extract_team_meta_from_task(task: &ToolCall) -> Option<TeamMeta>`：从 Task input 提取 team_name + member_name
- [x] 4.2 单元测试：有 team 信息的 Task、无 team 信息的 Task

## 5. build_chunks 接入

- [x] 5.1 修改 `chunk/builder.rs` 的 `MessageCategory::User` 分支：增加 `is_teammate_message` guard，跳过 teammate 消息
- [x] 5.2 新增 `build_chunks_with_subagents(messages, candidates) -> Vec<Chunk>`：在 `pair_tool_executions` 后调用 `resolve_subagents` + `filter_resolved_tasks`
- [x] 5.3 在 `lib.rs` 导出 `build_chunks_with_subagents`
- [x] 5.4 单元测试：teammate 消息不产出 `UserChunk`；`build_chunks_with_subagents` 过滤已 resolve 的 Task

## 6. lib.rs 导出 + 集成

- [x] 6.1 在 `team/mod.rs` 和 `lib.rs` 通过 `pub use` 导出公开 API
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.3 `cargo fmt --all`
- [x] 6.4 `cargo test -p cdt-analyze` 全测试通过

## 7. 文档 + 收尾

- [x] 7.1 更新根 `CLAUDE.md` 的 Capability→crate map：`team-coordination-metadata` → `done ✓`
- [x] 7.2 更新 `CLAUDE.md` 脚注 †：移除"端到端接入留给 `port-team-coordination-metadata`"
- [x] 7.3 `openspec validate port-team-coordination-metadata --strict`

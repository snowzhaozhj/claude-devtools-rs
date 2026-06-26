## Why

用 cdt CLI/MCP 分析 Claude Code 会话反而不如 AI 直接解析 JSONL：工具多（6 个）、参数多（31 个）、`content_mode=omit` 默认逼 AI 多次调用才看到内容、且没有查看完整工具调用链的接口。为省 token 的设计因每次调用重复 system prompt 开销，反而比直接返回完整数据更贵（实测某会话 27 次调用 ~100K token 后 AI 放弃 API 自行解析）。本 change 把 API 从「为 UI 设计的骨架 + 懒加载」重设计为「为 AI 设计：一次拿到完整相关数据」。

> **落盘说明**：本 change 是 explore 阶段充分讨论后的设计落盘。实现依赖前置 bug **#540**（turn 锚定）先修复（见 Impact）；在 #540 落地前本 change 停在 propose 阶段，不进 apply。

## What Changes

- **BREAKING** MCP 工具集从 6 个重构为 7 个：`list_projects` / `list_sessions` / `get_session` / **`get_turn`（新）** / **`get_tool_output`（新）** / `search` / `get_stats`；删除 `get_session_chunks`。总参数 31 → 23。
- **BREAKING** `get_session` 语义重定义：从「summary + cost + errors」改为 **compact overview**——按 turn 返回 question + answer + 工具聚合 + metrics（不再是 chunk 骨架）。
- 引入 **turn 数据模型**：turn = 一条真实用户消息 + 它的 AI 响应；`get_turn` 返回单 turn 完整 steps（thinking / text / tool / subagent 等）；`get_tool_output` 取被截断 step 的完整原文（API 自闭环）。
- 删除「为省 token」的反模式参数：`content_mode` / `include` / `filter` / `max_chunks` / `grep_context`；删除客户端可自行过滤的 `group_by` / `branch` / `is_ongoing` / `limit`；删除可服务端自动解析的 `project`（get_session）。
- 服务端内置截断替代参数化裁剪：question / answer / thinking 全量；tool output ≥5KB 截前 2000 字（`get_tool_output` 取全文）。
- 分页统一为 `total` + `nextCursor`（对齐 MCP 协议约定）。
- `search` 返回 **turn 级命中**（sessionId + turnIndex + matchSnippet），AI 可直接 `get_turn` 钻取。
- CLI 与 MCP **数据参数完全一致**；CLI 仅额外保留终端渲染 flags（`--format` / `--json` / `--no-truncate` / `--raw`）。

## Capabilities

### New Capabilities
- `session-turn-view`: turn/step 数据模型的单一 owner——turn 边界定义、step 类型集合（镜像桌面端 DisplayItem union）、tool output 三态（text / structured / missing）、服务端截断规则、`total`+`nextCursor` 分页契约。`mcp-server` / `cli-output` / `session-search` 引用它，不各自重复定义 turn/step 字段。

### Modified Capabilities
- `mcp-server`: 工具集 6 → 7（删 `get_session_chunks`，加 `get_turn` / `get_tool_output`，重定义 `get_session`），参数精简，删 `content_mode` / `include` / `filter` 等；`get_session_chunks` 的 grep / context budget 行为迁移到新工具。
- `cli-output`: 删 `content_mode` / `--all` / `range`-`tail` 互斥 / `--extract`-tools 等围绕 chunk 的输出契约；新增 `cdt turn` / `cdt tool-output` 子命令；session 默认输出改 turn 视图，`--raw` 保留原 chunk 逃生舱。

> `session-search` 的匹配语义（索引什么内容、mtime 遍历、子串匹配）**不变**（见 design D10），故不列为 Modified Capability；命中结果映射到 turn 级（turnIndex）的呈现契约由新 capability `session-turn-view` owner，`search` 工具引用它。

## Impact

- **前置 blocker #540**：现有 turn 锚定在 `AIChunk`（每个 AIChunk +1），导致被打断的用户消息（响应写为 `model=<synthetic>` 占位、被 hard-noise 过滤、不产 AIChunk）从 turn 视图丢失（语料实测 ~597/9095 ≈ 6.5% 真实消息丢 turn）。本 change 的 turn 模型要忠实，必须先把 turn 锚定改为「真实用户消息」。诊断工具 `crates/cdt-api/tests/corpus_turn_fidelity.rs`（`investigate/turn-anchoring` 分支）。
- 代码影响锁在 CLI/MCP 专属层：`cdt-query`（turn 构建 + subagent 解析）+ `cdt-cli`（view 层 + MCP 工具 + CLI 子命令）。桌面端不 link `cdt-query`，零性能影响；`cdt-core` / `cdt-parse` / `cdt-analyze` 及 `cdt-api::get_session_detail` hot path 不改（只消费已有输出）。`buildDisplayItems`（AIChunk → steps + answer）从前端 TS port 到 Rust 共享。
- **BREAKING** 现有依赖 `get_session_chunks` / `content_mode` 的 MCP 客户端、CLI 调用、skill 需迁移。
- 性能：CLI/MCP 直接调 `cdt_parse` + `cdt_analyze` 拿完整数据，不走 IPC 的 `OMIT_TOOL_OUTPUT`；persisted-output 占位符在构建 turn 时透明回读。

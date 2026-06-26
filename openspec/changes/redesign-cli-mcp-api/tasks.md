## 0. 前置 blocker（已完成）

- [x] 0.1 #540 turn 锚定改为「真实用户消息锚定」落地（commit 7db5c526）
- [x] 0.2 corpus_turn_fidelity 确认丢 turn 趋近 0（corpus_q2_aionly.rs 守卫）
- [x] 0.3 #542 derive_turns 一等公民落地（commit 0f30c9e4），turn 模型一致

## 1. step 构建逻辑（Rust port）

- [x] 1.1 把前端 `buildDisplayItems`（AIChunk → steps + answer）port 到 Rust（落 `cdt-query`），含 step 类型 union（12 种，含 compaction/system）+ tool merge + answer 判定
- [x] 1.2 消费 `cdt-analyze::derive_turns` 的 `Turn.member_chunk_ids` 组装 per-turn step 列表（AIChunk → 常规 steps，CompactChunk → compaction step，SystemChunk → system step）
- [x] 1.3 单测：step 类型 / tool output 三态 / answer 边界 / 截断规则

## 2. cdt-query::Engine 扩展（CLI/MCP 专属层）

- [x] 2.1 Engine 加 turn 视图构建（compact overview + 单 turn 完整 steps）
- [x] 2.2 服务端截断（tool output ≥5KB）+ get_tool_output 按 toolUseId 取全文（复用已有 `DataApi::get_tool_output`）
- [x] 2.3 turn metrics 计算（token usage / cost / durationMs / model）
- [x] 2.4 grep matchedIn 归因：chunk 级匹配后二次扫描确定命中来源（tool:<name> / answer / question / thinking / error），取优先级最高的
- [x] 2.5 确认不走 IPC 的 `OMIT_TOOL_OUTPUT`，不改 `cdt-api::get_session_detail` hot path
- [ ] ~~2.6 subagent lazy 反查索引~~ **deferred**（D15b）

## 3. view 层（cdt-cli）

- [x] 3.1 新增 TurnCompact / Step / ToolOutputView view structs（serde camelCase）
- [x] 3.2 统一分页响应：`total` + `nextCursor` + `pageSize` 参数（直接写，不抽象）
- [x] 3.3 search 命中映射到 turn 级（sessionId + turnIndex + matchSnippet）

## 4. MCP 工具集（cdt-cli/src/mcp）

- [x] 4.1 删 `get_session_chunks`；新增 `get_turn` / `get_tool_output`
- [x] 4.2 重定义 `get_session` 返回 compact overview（turn 列表）
- [x] 4.3 精简各工具参数（删 content_mode/include/filter/max_chunks/grep_context/group_by/branch/is_ongoing/limit/project；加 pageSize）
- [x] 4.4 更新 MCP server instructions（intent → 新工具映射）
- [x] 4.5 MCP 字段契约测试同步

## 5. CLI 子命令（cdt-cli/src/main）

- [x] 5.1 新增 `cdt turn <id> <n>` / `cdt tool-output <id> <toolUseId>` 子命令
- [x] 5.2 `cdt session <id>` 默认输出改 turn 视图；`--raw` 保留原 chunk 逃生舱
- [x] 5.3 确认 CLI 数据参数与 MCP 完全一致，仅终端渲染 flags（--format/--json/--no-truncate/--raw）为 CLI-only

## 6. 迁移与文档

- [x] 6.1 更新依赖旧工具的 setup / skill / 文档（session-insights 等）
- [x] 6.2 CHANGELOG（用户可感知：API 重设计 / BREAKING）

## 7. 验证（真实数据，CLI + MCP 双入口）

- [x] 7.1 会话 514 / 8e69be84 等用例：新 API 2-3 次调用拿到完整调用链
- [x] 7.2 大 turn（>50 steps）分页 + 大 tool output 截断 + get_tool_output 全文
- [ ] 7.3 ~~subagent 递归钻取真实验证~~ **deferred**（D15b）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（BREAKING + 跨 capability，高风险禁豁免）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再 wait-ci 全绿）

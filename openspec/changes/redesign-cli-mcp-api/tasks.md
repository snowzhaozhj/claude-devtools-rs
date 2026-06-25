## 0. 前置 blocker（#540，非本 change 实现）

- [ ] 0.1 #540 turn 锚定改为「真实用户消息锚定」落地（含 `<synthetic>` 过滤是否过宽的裁定）
- [ ] 0.2 重跑 `crates/cdt-api/tests/corpus_turn_fidelity.rs`，确认「真实对话丢 turn」→ 趋近 0
- [ ] 0.3 #540 合并后，确认本 change 的 turn 模型与修正后的 turn 定义一致

## 1. 共享 turn 构建逻辑（Rust）

- [ ] 1.1 把前端 `buildDisplayItems`（AIChunk → steps + answer）port 到 Rust（落 `cdt-analyze` 或 `cdt-query`），含 step 类型 union + tool merge + answer 判定
- [ ] 1.2 实现 turn 配对（真实用户消息 + 其 AI 响应），turnIndex 复用 `cdt-analyze` 修正后的 turn 定义
- [ ] 1.3 persisted-output 占位符在构建 turn 时透明回读真实文件内容
- [ ] 1.4 单测：step 类型 / tool output 三态 / answer 边界 / 截断规则

## 2. cdt-query::Engine 扩展（CLI/MCP 专属层）

- [ ] 2.1 Engine 加 turn 视图构建（compact overview + 单 turn 完整 steps）
- [ ] 2.2 Engine 加 subagent lazy 反查索引（subagentSessionId → 父 session），使 get_session/get_turn 可解析 subagent 子目录
- [ ] 2.3 服务端截断（tool output ≥5KB）+ get_step_output 取全文路径
- [ ] 2.4 确认不走 IPC 的 `OMIT_TOOL_OUTPUT`，不改 `cdt-api::get_session_detail` hot path

## 3. view 层（cdt-cli）

- [ ] 3.1 新增 TurnCompact / Step / StepOutput view structs（serde camelCase）
- [ ] 3.2 统一分页响应：`total` + `nextCursor`（直接写，不抽象）
- [ ] 3.3 search 命中映射到 turn 级（sessionId + turnIndex + matchSnippet）

## 4. MCP 工具集（cdt-cli/src/mcp）

- [ ] 4.1 删 `get_session_chunks`；新增 `get_turn` / `get_step_output`
- [ ] 4.2 重定义 `get_session` 返回 compact overview（turn 列表）
- [ ] 4.3 精简各工具参数（删 content_mode/include/filter/max_chunks/grep_context/group_by/branch/is_ongoing/limit/project）
- [ ] 4.4 更新 MCP server instructions（intent → 新工具映射）
- [ ] 4.5 ipc_contract / MCP 字段契约测试同步

## 5. CLI 子命令（cdt-cli/src/main）

- [ ] 5.1 新增 `cdt turn <id> <n>` / `cdt step-output <id> <t> <s>` 子命令
- [ ] 5.2 `cdt session <id>` 默认输出改 turn 视图；`--raw` 保留原 chunk 逃生舱
- [ ] 5.3 确认 CLI 数据参数与 MCP 完全一致，仅终端渲染 flags（--format/--json/--no-truncate/--raw）为 CLI-only

## 6. 迁移与文档

- [ ] 6.1 更新依赖旧工具的 setup / skill / 文档（session-insights 等）
- [ ] 6.2 CHANGELOG（用户可感知：API 重设计 / BREAKING）

## 7. 验证（真实数据，CLI + MCP 双入口）

- [ ] 7.1 会话 514 / 8e69be84 等用例：新 API 2-3 次调用拿到完整调用链
- [ ] 7.2 subagent 递归钻取真实验证
- [ ] 7.3 大 turn（>50 steps）分页 + 大 tool output 截断 + get_step_output 全文

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（BREAKING + 跨 capability，高风险禁豁免）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再 wait-ci 全绿）

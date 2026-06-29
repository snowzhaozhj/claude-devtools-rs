# Tasks: export-missing-displayitems

## 1. 后端 subagent messages 封顶填充

- [x] 1.1 在 `cdt-api` 新增 pub `cap_subagent_messages(chunks, MAX_SUBAGENT_DEPTH=1, MAX_BYTES_PER_SUBAGENT=2MiB, MAX_EXPORT_SUBAGENT_TOTAL_BYTES=50MiB)`：① 递归清空深度 > max_depth 的嵌套 subagent.messages（+ omitted）；② 对每个保留 subagent 按清空后形态真实 `serde_json::to_vec` 字节量，超 per-subagent cap 则清空 + omitted（单个独立，不计入全局）；③ 按 chunks 顺序累计未清空者字节，超全局 cap 后续清空 + omitted
- [x] 1.2 `apply_export_omissions` 用 `cap_subagent_messages` 替代 `OMIT_SUBAGENT_MESSAGES` 顶层清空；`apply_display_omissions` 保持全清空不变
- [x] 1.3 后端单测：depth-cap（嵌套子代理清空）、per-subagent byte cap（病态单个清空 + omitted）、全局 cap 兜底（多个未超 per-subagent 但累计超全局 → 后续清空）、上限内保留、三闸门顺序（depth→per-subagent→global）、单个巨型 subagent 不影响其他 subagent 保留
- [x] 1.4 验证 `engine.get_session_detail`（CLI）+ HTTP route 返回的 subagent.messages 递归层与 workflow_items 是否填充（决定 CLI/HTTP 渲染或降级）

## 1b. 浏览器 HTTP 导出路径

- [x] 1b.1 HTTP `get_session_detail` route 加 `?export=1` query 分支：见 export=1 时对结果调 `apply_export_omissions`（含 cap），否则现状不变（首屏完整）
- [x] 1b.2 `transport.ts` 把 `get_session_detail_for_export` 映射改为 `/api/sessions/{id}?export=1`
- [x] 1b.3 HTTP route 测试：export=1 返回 cap 后数据 + 首屏（无参数）行为不变

## 2. 前端 exporter 四类 + subagent 内部对话渲染

- [x] 2.1 `projection.ts::ProjectedSessionDetail` 增加 `workflowItems` 透传字段
- [x] 2.2 `markdownExporter.ts`：`slash` / `teammate_message` / `teammate_spawn` 三 case 补渲染；`tool` case 内 `workflowRunId` 命中 workflowItems 且 runId 未渲染过时渲染 workflow 摘要替代 tool，记入 seen set，同 runId 后续 tool 跳过
- [x] 2.3 `markdownExporter.ts::renderSubagent`：`sub.messages` 非空时**先对 messages 应用同一 `projectChunk(options)`**，再 `buildDisplayItemsFromChunks` 递归渲染；`messagesOmitted` 时标注内部对话已省略
- [x] 2.4 `htmlExporter.ts`：同 2.2 + 2.3 的 HTML 版渲染（含 workflow 去重 + 递归前 project）
- [x] 2.5 exporter 调用链把 workflowItems 构建成 `Map<runId, WorkflowItem>` + seen set 贯穿单次导出，传入 renderDisplayItem

## 3. CLI exporter 四类 + subagent 内部对话渲染

- [x] 3.1 `main.rs::export` 移除 `filtered_detail` 的 `workflow_items: vec![]`，透传 `session_detail.workflow_items`（若 1.4 验证未填充则降级 + 记 deferred）
- [x] 3.2 CLI export 路径调用 `cap_subagent_messages`（同桌面 depth + per-subagent byte cap，正常文件输出不触发，仅病态截断）
- [x] 3.3 `export.rs::render_ai_chunk_md`：补 slash（`ai.slash_commands`）/ teammate_message（`ai.teammate_messages`）/ teammate_spawn（`te.teammate_spawn`）/ workflow（`workflow_items` map + runId 去重）渲染
- [x] 3.4 `export.rs::render_subagent_md`：递归渲染 `sub.messages`（复用 `render_chunk_md`，递归前按 CLI options 过滤 thinking/detail），`messages_omitted` 标注省略

## 4. 测试与验证

- [x] 4.1 `export.test.ts` 补 slash / teammate_message / teammate_spawn / workflow 渲染断言（markdown + html）
- [x] 4.2 `export.test.ts` 补 subagent 内部对话递归渲染 + messagesOmitted 标注断言
- [x] 4.3 CLI export 测试补四类 + subagent messages 递归 + 封顶省略
- [x] 4.4 `cargo test -p cdt-api -p cdt-cli` + `just test-ui-unit` 全绿
- [x] 4.5 `pnpm --dir ui run check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all`
- [x] 4.6 真数据验证：起 `cdt` HTTP server，导出含 subagent/teammate/workflow/slash 的会话，核对 markdown/html 内容非空且对齐视图（e2e-http-verify）
- [x] 4.7 `openspec validate export-missing-displayitems --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

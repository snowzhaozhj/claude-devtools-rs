# Tasks

## 1. 后端：导出专用裁剪 + command（Bug 2，D3/D4）

- [x] 1.1 `crates/cdt-api/src/ipc/local.rs`：新增 `pub(crate) fn apply_export_omissions(chunks)` = 仅 `apply_image_omit` + 清空 subagent `messages`（设 `messages_omitted=true`），**不**裁剪 tool-output、**不**裁剪 response-content
- [x] 1.2 `crates/cdt-api/src/ipc/types.rs`：`SessionDetailResponse` 新增 `apply_export_omissions(&mut self)` 方法（对 `Full` variant 调 `local::apply_export_omissions`）
- [x] 1.3 `src-tauri/src/lib.rs`：新增 `#[tauri::command] async fn get_session_detail_for_export(data, project_id, session_id)` —— 调 `api.get_session_detail(.., None)` 后调 `resp.apply_export_omissions()` 再 `serde_json::to_value` 返回；注册进 `invoke_handler!`
- [x] 1.4 `cargo clippy --workspace --all-targets -- -D warnings`（含 src-tauri manifest）+ `cargo fmt --all`

## 2. 后端：IPC contract 同步（硬约束，D5）

- [x] 2.1 `crates/cdt-api/tests/ipc_contract.rs`：`EXPECTED_TAURI_COMMANDS` 加 `get_session_detail_for_export`
- [x] 2.2 contract test：构造含 tool output / response content / image / subagent messages 的 `SessionDetailResponse::Full`，断言 `apply_export_omissions` 后 tool-output + response-content 保留（`outputOmitted`/`contentOmitted` 非 true）、image + subagent-messages 裁剪（`dataOmitted`/`messagesOmitted` 为 true）；`apply_omissions` 后四项全裁剪（首屏行为不变）
- [x] 2.3 `cargo test -p cdt-api --test ipc_contract`

## 3. 前端：导出器时序合并（Bug 1，D1/D2/D7）

- [x] 3.1 `ui/src/lib/export/markdownExporter.ts::renderAIChunk`：改为 `const { items, lastOutput } = buildDisplayItems(projectedChunk)`，按 `items` 顺序渲染（thinking / output / tool / subagent / user_message），末尾渲染 `lastOutput`；删除原"末尾统一堆 toolExecutions / subagents"两个 loop；未覆盖的 DisplayItem 类型（slash / teammate / workflow）走 default 跳过
- [x] 3.2 `ui/src/lib/export/htmlExporter.ts::renderAIHtml`：同 3.1 改造
- [x] 3.3 用**非缓存** `buildDisplayItems`（非 `buildDisplayItemsCached`）——避免投影后 chunk 与视图缓存撞键（design D2）
- [x] 3.4 `ui/src/lib/export/projection.ts::projectSubagents`：`includeSubagents=false` 改为**返回 `[]`**（整体丢弃 subagents），而非仅清空 messages——让 `buildDisplayItems` 不跳过 Task 工具（design D7）

## 4. 前端：改调导出专用 command + transport 分叉（Bug 2，D5/D6）

- [x] 4.1 `ui/src/lib/api.ts`：新增 `getSessionDetailForExport(projectId, sessionId)`——Tauri 走 `invoke("get_session_detail_for_export", {...})`，HTTP/浏览器复用既有 `getSessionDetail(projectId, sessionId, null)`；返回类型同 `getSessionDetail`
- [x] 4.2 `ui/src/components/SessionMetaMenu.svelte::doExport`：改调 `getSessionDetailForExport`
- [x] 4.3 `ui/src/lib/transport.ts`：为 Tauri 分支登记 `get_session_detail_for_export`（确认 command 映射存在）
- [x] 4.4 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`：加 `get_session_detail_for_export`（mock 返回完整未裁剪 detail）
- [x] 4.5 `ui/src/lib/__fixtures__/*`：无需改动——既有 fixture 已含完整 tool output

## 5. 前端测试

- [x] 5.1 Vitest：导出器单测——构造「文本 A → 工具 T → 文本 B」+ subagent 的 AIChunk，断言导出顺序穿插正确（Bug 1 回归）
- [x] 5.2 Vitest：断言 full 模式工具 output 非空渲染（Bug 2 回归）；JSON 导出 `responses[].content` 非空、`contentOmitted` 非 true
- [x] 5.3 Vitest：`includeSubagents=false` + Task/Agent + subagent_spawn → 断言 Task 工具仍渲染、无 subagent 卡片（D7 回归）
- [x] 5.4 反转 fix 验证：临时改回 buggy 实现跑测试应 fail，改回 fix 应 pass（debug-first 硬约束）
- [x] 5.5 `just test-ui-unit` + `pnpm --dir ui run check`

## 6. 验证与收尾

- [x] 6.1 `openspec validate fix-export-tool-order-and-output --strict`
- [ ] 6.2 手动 `just dev`：桌面端导出 Markdown/JSON/HTML，确认顺序正确 + output 非空
- [x] 6.3 开 GitHub issue：teammate / slash / workflow 内容导出缺失（范围外，`bug` label）#534
- [x] 6.4 CHANGELOG `## [Unreleased] ### Fixed` 追加导出修复条目（英文）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

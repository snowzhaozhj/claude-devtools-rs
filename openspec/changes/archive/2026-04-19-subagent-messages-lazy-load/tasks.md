# Tasks

## 1. cdt-core: Process 加 derived 字段

- [x] 1.1 `crates/cdt-core/src/process.rs` `Process` struct 加 4 字段：`header_model: Option<String>`、`last_isolated_tokens: u64`、`is_shutdown_only: bool`、`messages_omitted: bool`，全部 `#[serde(default)]`
- [x] 1.2 同步更新 `process_roundtrip_defaults` / `process_roundtrip_full` 测试覆盖新字段
- [x] 1.3 加单测 `process_messages_omitted_serializes_camel_case` 验证 camelCase

## 2. cdt-analyze: 派生字段计算

- [x] 2.1 `crates/cdt-analyze/src/tool_linking/resolver.rs` 新增 pure 函数 `derive_subagent_header(messages: &[Chunk]) -> (Option<String>, u64, bool)`
- [x] 2.2 `header_model`：找最后一条 AI 最后一条 response.model 跑 `simplify_model_name`
- [x] 2.3 `last_isolated_tokens`：累加最后一条 AI 最后一条 response.usage 4 项
- [x] 2.4 `is_shutdown_only`：单 assistant + 单 SendMessage + input.type==shutdown_response
- [x] 2.5 `candidate_to_process` 调 `derive_subagent_header` 并填充 4 字段
- [x] 2.6 单测：`derive_subagent_header_picks_last_ai_model_simplified` / `derive_subagent_header_sums_last_usage` / `derive_subagent_header_detects_shutdown_only` / `derive_subagent_header_multiple_assistants_not_shutdown_only`

## 3. cdt-api: payload 裁剪 + 新 IPC

- [x] 3.1 `crates/cdt-api/src/ipc/local.rs` 顶部加 `const OMIT_SUBAGENT_MESSAGES: bool = true;`
- [x] 3.2 `LocalDataApi::get_session_detail` 序列化前 clone chunks 把 subagent.messages 替换为空 + 设 `messages_omitted=true`
- [x] 3.3 `crates/cdt-api/src/ipc/traits.rs::DataApi` 加 `async fn get_subagent_trace(&self, root_session_id: &str, subagent_session_id: &str) -> Result<Value, ApiError>`，默认实现返回空数组
- [x] 3.4 `LocalDataApi::get_subagent_trace`：扫所有 project 找 root session 所在目录后用 `find_subagent_jsonl` 定位 + `parse_file` + `build_chunks`
- [ ] 3.5 集成测试 `tests/lazy_subagent.rs`（**用 perf bench 测试已验证 raw 7702 KB → IPC 3070 KB；fixture 测试可作为后续加固，本次以实测数据为准**）

## 4. src-tauri: 注册新 command

- [x] 4.1 `src-tauri/src/lib.rs` 加 `#[tauri::command] async fn get_subagent_trace(...)`
- [x] 4.2 注册到 `invoke_handler!`
- [x] 4.3 `cargo build --manifest-path src-tauri/Cargo.toml` 通过

## 5. 前端：Process TS 类型 + api.ts

- [x] 5.1 `ui/src/lib/api.ts` 的 `SubagentProcess` 类型加 `headerModel?: string | null`、`lastIsolatedTokens?: number`、`isShutdownOnly?: boolean`、`messagesOmitted?: boolean`
- [x] 5.2 加 `getSubagentTrace(rootSessionId, subagentSessionId): Promise<Chunk[]>`

## 6. SubagentCard 改造

- [x] 6.1 加必传 prop `rootSessionId: string`
- [x] 6.2 加 `messagesLocal: Chunk[] | null = $state(null)` + `isLoadingTrace: boolean`
- [x] 6.3 `modelName` 优先 `process.headerModel` fallback 派生
- [x] 6.4 `isolatedTokens` 优先 `process.lastIsolatedTokens` (>0) fallback
- [x] 6.5 `isShutdownOnly` 优先 `process.isShutdownOnly` fallback（注意未定义而非 false 才走 fallback，`undefined` 检查）
- [x] 6.6 `traceItems` 用 `effectiveMessages = messagesLocal ?? process.messages`
- [x] 6.7 `toggleExpanded` 改 async；展开时 `await ensureMessages()`
- [x] 6.8 `ensureMessages()`：localCache hit / 非 omitted / 真懒拉 三路分支
- [x] 6.9 trace 加载中显示 "Loading trace…" 占位（CSS `.sa-trace-loading`）
- [x] 6.10 嵌套 `<SubagentCard>` 渲染时把 `rootSessionId` 一路传递（含 `ExecutionTrace.svelte` 加 prop）

## 7. SessionDetail 传参

- [x] 7.1 `<SubagentCard process={item.process} rootSessionId={sessionId} />`

## 8. 验证

- [x] 8.1 `cargo build --workspace` + `cargo test --workspace` 全绿
- [x] 8.2 `cargo build --manifest-path src-tauri/Cargo.toml` 通过
- [x] 8.3 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 8.4 `cargo fmt --all`
- [x] 8.5 `npm run check --prefix ui` 通过（5 个既有 warning，无新增 error）
- [x] 8.6 重跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture`：46a25772 case 实测 raw 7702 KB → IPC 3070 KB（砍 60%），4cdfdf06 3472→1768 (49%)，7826d1b8 5161→4840 (6%, 仅 1 sub)
- [ ] 8.7 `just dev` 启动验证 console `[perf]` IPC 数据降到 < 250 ms（**留待用户验证**：探针已就位）
- [ ] 8.8 验证嵌套 subagent 各自展开各自拉 trace（**留待用户验证**）
- [ ] 8.9 `OMIT_SUBAGENT_MESSAGES = false` 重启验证回滚生效（**留待用户验证**）

## 9. spec + followups + archive

- [x] 9.1 `openspec validate subagent-messages-lazy-load --strict` 通过
- [x] 9.2 `just preflight` 全绿
- [x] 9.3 `openspec/followups.md` 更新"性能 / 首次打开大会话卡顿"条目：phase 2 已落地 + 实际收益数据
- [ ] 9.4 `openspec archive subagent-messages-lazy-load -y`（下一步执行）
- [ ] 9.5 `CLAUDE.md` "UI 已知遗留问题" 同步引用归档 slug
- [ ] 9.6 更新 memory `project_perf_large_session.md`：phase 2 已落地，效果观察中

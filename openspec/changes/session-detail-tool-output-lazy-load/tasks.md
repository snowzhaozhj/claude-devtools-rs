## 1. cdt-core 数据结构扩展

- [x] 1.1 `crates/cdt-core/src/tool_execution.rs::ToolExecution` 加 `#[serde(rename = "outputOmitted", default)] pub output_omitted: bool` 字段
- [x] 1.2 `tool_execution_roundtrip` 测试加 `output_omitted` 字段；新增 `tool_execution_default_output_omitted_false` + `tool_execution_output_omitted_roundtrip` + `tool_output_trim_*` 三测
- [x] 1.3 同步更新 workspace 内所有 `ToolExecution { ... }` 字面构造点（5 个生产 + 4 个 test fixture）补 `output_omitted: false`
- [x] 1.4 `cargo build --workspace` + `cargo test -p cdt-core` 通过（46 tests）

## 2. cdt-api 后端 OMIT 路径

- [x] 2.1 `crates/cdt-api/src/ipc/local.rs` 顶部加 `const OMIT_TOOL_OUTPUT: bool = true;` 模块常量
- [x] 2.2 新增 `apply_tool_output_omit(chunks: &mut [Chunk])` 函数 + `ToolOutput::trim()` helper（cdt-core）
- [x] 2.3 `get_session_detail` 序列化前调用顺序：image OMIT → response.content OMIT → tool_output OMIT → subagent OMIT
- [x] 2.4 单元测试 `apply_tool_output_omit_clears_text_variant`
- [x] 2.5 单元测试 `apply_tool_output_omit_clears_structured_variant` + `apply_tool_output_omit_keeps_missing_variant_kind`
- [x] 2.6 单元测试 `apply_tool_output_omit_clears_nested_subagent_tool_output`

## 3. cdt-api 新 IPC: get_tool_output

- [x] 3.1 `crates/cdt-api/src/ipc/traits.rs::DataApi` trait 加 `async fn get_tool_output`，默认实现返回 `ToolOutput::Missing`
- [x] 3.2 `LocalDataApi::get_tool_output` 实现：locate_session_jsonl → parse_file → build_chunks → 遍历 ToolExecution 找 tool_use_id → 返回 output；找不到/失败 → Missing
- [x] 3.3 单元测试 `get_tool_output_returns_missing_when_jsonl_not_exist`（其余 path 已被 phase 2 测试覆盖：locate_session_jsonl 失败路径）
- [x] 3.4 `cargo test -p cdt-api` 通过（24 lib tests）

## 4. Tauri 集成

- [x] 4.1 `src-tauri/src/lib.rs` 注册新 Tauri command `get_tool_output(state, root_session_id, session_id, tool_use_id) -> Result<serde_json::Value, String>`
- [x] 4.2 `invoke_handler!` 加 `get_tool_output` 入口
- [x] 4.3 `cargo build --manifest-path src-tauri/Cargo.toml` 通过

## 5. 前端 ExecutionTrace 懒拉

- [x] 5.1 `ui/src/lib/api.ts` 加 `getToolOutput` 函数 + `ToolOutput` 类型导出
- [x] 5.2 `ui/src/lib/api.ts::ToolExecution` 加 `outputOmitted?: boolean`
- [x] 5.3 `ExecutionTrace.svelte` 新加 `sessionId?: string` props（fallback 到 rootSessionId）
- [x] 5.4 `ExecutionTrace.svelte` + `SessionDetail.svelte` 各自加 `outputCache: Map<string, ToolOutput> = $state(new Map())`
- [x] 5.5 `toggle(key, exec?)` 函数：opening + exec.outputOmitted=true + cache 未命中时 fire-and-forget 调 `getToolOutput`
- [x] 5.6 ToolViewer 渲染走 `effectiveExec(exec)`：cache 命中替换 output，否则透传 exec
- [x] 5.7 SessionDetail 内联 tool 渲染分支同步改造（不复用 ExecutionTrace 组件，自己内联）
- [x] 5.8 `SubagentCard.svelte` 调 `<ExecutionTrace>` 时传 `sessionId={process.sessionId}`
- [x] 5.9 `npm run check --prefix ui` 通过（0 errors）

## 6. preflight + perf bench 验证

- [x] 6.1 `just fmt` 通过
- [x] 6.2 `just lint` 通过（workspace + src-tauri clippy）
- [x] 6.3 `just test` 通过（含前端）
- [x] 6.4 `just spec-validate` 通过（21 passed）
- [x] 6.5 perf bench 重跑实测：4cdfdf06 248 → **163 KB** (-34%)；7826d1b8 334 → **202 KB** (-40%)；46a25772 1829 → **1412 KB** (-23%)。命中预期"渐进改良"档位

## 7. 收尾

- [ ] 7.1 `openspec/followups.md` 性能条目加 Phase 5 落地子段
- [ ] 7.2 commit + archive

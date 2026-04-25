# tasks: tool-output-omit-preserve-size

## 1. cdt-core: 字段定义

- [ ] 1.1 `ToolExecution` 加 `pub output_bytes: Option<u64>`，serde camelCase + `#[serde(default)]`
- [ ] 1.2 现有测试 fixture / `ToolExecution::default` 等位补 `output_bytes: None`
- [ ] 1.3 新增 `tool_execution_output_bytes_roundtrip` 单测覆盖 Some(N) / None 双向

## 2. cdt-api: OMIT 层填值

- [ ] 2.1 `apply_tool_output_omit` 在 `trim` 前按 variant 记录 size：`Text` → `text.len()`、`Structured` → `serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)`、`Missing` 跳过
- [ ] 2.2 单测扩展：`apply_tool_output_omit_clears_text_variant` 加 assert `output_bytes == Some(text.len())`；`apply_tool_output_omit_clears_structured_variant` 同理；`apply_tool_output_omit_keeps_missing_variant_kind` assert `output_bytes == None`

## 3. 前端：fallback 链 + UI 接入

- [ ] 3.1 `ui/src/lib/api.ts::ToolExecution` 接口加 `outputBytes?: number`
- [ ] 3.2 `ui/src/lib/toolHelpers.ts::getToolOutputTokens`：优先用 `exec.outputBytes` 算（按 4 字符/token），仅在 `outputBytes == null` 时 fallback 到 `output.text.length` / `JSON.stringify(output.value).length`
- [ ] 3.3 `ExecutionTrace.svelte` / `SessionDetail.svelte` tool 行的 `tokenCount` 改回 `getToolInputTokens(exec) + getToolOutputTokens(exec)`（合计），用 `exec` 而非 `eff`——展开前后稳定
- [ ] 3.4 svelte-check 0 errors

## 4. spec delta + 验证

- [ ] 4.1 `openspec/changes/tool-output-omit-preserve-size/specs/ipc-data-api/spec.md` MODIFIED `Expose project and session queries` 加 outputBytes 子句 + Scenario
- [ ] 4.2 `openspec validate tool-output-omit-preserve-size --strict` 通过
- [ ] 4.3 `cargo test --workspace` + `npm run check --prefix ui` 全绿

## 5. archive

- [ ] 5.1 `openspec archive tool-output-omit-preserve-size -y` 同步主 spec

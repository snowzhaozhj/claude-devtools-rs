# proposal: tool-output-omit-preserve-size

## Why

`get_session_detail` 在 `OMIT_TOOL_OUTPUT=true` 默认路径会把 `ToolOutput::Text.text`
设为 `""` / `Structured.value` 设为 `Null`（见 `cdt-core::ToolOutput::trim`），同时
设 `outputOmitted=true`。这让前端**完全丢失**了原始 output 的大小信息，导致
两个体验问题：

1. **token 数随展开抖动**：`BaseItem` 头部显示的 tool token 数依赖 `effectiveExec`，
   懒加载前 output token = 0、懒加载后 output token = N；展开瞬间数字变大，用户
   反馈"展开前后统计的 token 不一样"
2. **token 数信息缺失**：要么显示 `input only`（output 永远不可见，又抹掉了
   "Output 工具有 N tokens"的信息），要么显示合计（跳变）。两难。

## What Changes

`ToolExecution` 新增 `output_bytes: Option<u64>` 字段（serde camelCase
`outputBytes`），语义为"原始 output 字符串字节长度"：

- 解析层（`cdt-parse` / `cdt-analyze::tool_linking`）SHALL **不**主动填充——保持
  `None`，不影响非 IPC 路径
- IPC OMIT 层（`cdt-api::ipc::local::apply_tool_output_omit`）SHALL **在** `trim`
  **前**记录 `text.len()` / `serde_json::to_string(value).len()` 到
  `output_bytes`，然后再 trim + 设 `output_omitted=true`
- 前端 `getToolOutputTokens` SHALL 优先读 `outputBytes ?? 4`（除以 4 字符/token
  启发式，与 `estimateTokens` 一致），fallback 到 `output.text.length` /
  `JSON.stringify(output.value).length`

效果：`BaseItem` 头部 token 数 = `getToolInputTokens(exec) + getToolOutputTokens(exec)`，
**懒加载前后相等**——展开不再跳变；同时仍能显示完整 context tokens（含 output）。

## Impact

- **Affected specs**：`ipc-data-api` MODIFIED `Expose project and session queries`
  （`OMIT_TOOL_OUTPUT` 段加 outputBytes 子句 + 1 个 Scenario）
- **Affected code**：
  - `crates/cdt-core/src/tool_execution.rs`：`ToolExecution` 加字段 + serde default
    `None` + roundtrip 测试
  - `crates/cdt-api/src/ipc/local.rs::apply_tool_output_omit`：trim 前记录 size
    + 单测两 case（text / structured 均填 outputBytes）
  - `ui/src/lib/api.ts`：`ToolExecution` 接口加 `outputBytes?: number`
  - `ui/src/lib/toolHelpers.ts::getToolOutputTokens`：优先用 outputBytes
- **Risk**：低；纯加字段、解析层不动；OMIT 层在 trim 前多算一次 size 开销可忽略
  （单 tool 一次 `String::len` / `to_string`）；前端 fallback 链兼容老后端
- **Backwards-compat**：`#[serde(default)]` 让旧 payload 不带 `outputBytes` 反序列化为
  `None`，前端 fallback 到 `output.text.length`——零破坏

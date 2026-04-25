# design: tool-output-omit-preserve-size

## Context

`get_session_detail` IPC 路径默认走 `OMIT_TOOL_OUTPUT=true` 把 `ToolExecution.output`
内的 `text` / `value` 字段裁空（保留 enum variant kind + 设 `output_omitted=true`）。
前端 `ExecutionTrace` 在用户点击展开时通过 `get_tool_output` IPC 按需懒拉。

但前端 `BaseItem` 头部需要在懒加载**前**就显示 tool token 数。当前 `getToolOutputTokens(exec)`
基于 `exec.output.text.length / 4` 启发式估算——OMIT 后 `text == ""` 导致返回 0，
**懒加载后** `effectiveExec` 替换为完整数据时返回真实值——展开瞬间数字跳变，用户报告
"展开前后 token 数不一样"。

要消除跳变，必须让前端在懒加载前就拿到 output 的"原始大小信息"。

## Decisions

### D1：字段加在 `ToolExecution` 而非 `ToolOutput` 内部

候选：
- (A) `ToolExecution` 加 `output_bytes: Option<u64>`（同级字段）
- (B) `ToolOutput::Text { text, original_bytes }` / `Structured { value, original_bytes }`
  在 enum variant 里加字段
- (C) 给 `ToolOutput` 加新 variant `Omitted { kind, original_bytes }`

选 **(A)**。理由：
- `ToolOutput` 已有现成的 `trim()` 方法语义（仅 inner 字段清空，variant kind 不变），
  在 variant 里加新字段会污染"`Text` 就是 `text` 字段"的简单结构，所有解析层 / 测试都要改
- (C) 引入新 variant 会破坏所有 `match` 穷举（filter / resolver / aggregator 都要加分支），
  且 `Omitted` 与 `output_omitted: bool` 信息重复
- (A) 与现有 `output_omitted` 是同模式（IPC 元数据字段，serde `default` 兼容旧 payload，
  解析层不主动写），扩展成本最低
- 风险：`ToolExecution` 字段数从 11 增至 12——可接受（其它 IPC OMIT 字段也是这种模式）

### D2：解析层不填，仅 IPC OMIT 层填

候选：
- (A) 解析层 `pair.rs::executions.push(ToolExecution { ... output_bytes: Some(text.len()) })`
  在 link 时就记录
- (B) 仅 `apply_tool_output_omit` 在 trim 前算，否则 `None`

选 **(B)**。理由：
- `output_bytes` 的语义是"OMIT 时给前端补失去的元信息"——解析层 output 仍完整时，前端
  直接 `output.text.length` 就够，没必要冗余
- 解析层不动 = `cdt-parse` / `cdt-analyze` 完全不感知此字段，回归测试不受影响
- 单元测试 fixture 也不需要批量加新字段（`#[serde(default)]` + Rust struct literal
  补 `output_bytes: None` 一次即可）
- HTTP 路径默认不走 OMIT，`output_bytes` 自然 `None`，前端 fallback 到原行为——零改动兼容

### D3：前端 fallback 链

`getToolOutputTokens(exec)`：
1. 若 `exec.outputBytes != null` → `Math.ceil(exec.outputBytes / 4)`（OMIT 路径或新后端）
2. 否则 `exec.output.kind === "text"` → `estimateTokens(exec.output.text)`（懒加载后 / HTTP / 老后端）
3. 否则 `exec.output.kind === "structured"` → `estimateContentTokens(exec.output.value)`
4. 否则 `0`

懒加载完成后 `outputBytes` 仍保留（OMIT 层填的是真实长度，懒加载只是补回 text）——
前端读 outputBytes 估算，与 OMIT 前的真实 `text.length / 4` 启发式**完全一致**——展开
前后数字稳定。

### D4：估算粒度（4 字符/token）

跟现有 `estimateTokens(text) = Math.ceil(text.length / 4)` 一致——这个启发式来自原版
`tokenFormatting.ts::estimateTokens`，本次不引入新算法。`u64` 字节长度足够覆盖
GiB 级 output（不会溢出 number 精度），前端按 `outputBytes / 4` 精度损失可忽略。

### D5：`Missing` variant 不填 `output_bytes`

orphan tool（`tool_use` 无对应 `tool_result`）的 output 是 `ToolOutput::Missing`——本身
就没有"原始字节长度"概念，`output_bytes` 保持 `None`。前端 fallback 到 0（match 落到
default case）。这与 OMIT 后 Missing variant 仍 Missing（见现有测试
`apply_tool_output_omit_keeps_missing_variant_kind`）一致——caller 看到 `output_omitted=true`
+ `Missing` + `output_bytes=None` 仍可触发懒拉，后端会返回 `Missing`，语义不变。

## Risks / Trade-offs

- **payload size**：每个 ToolExecution +8 byte（`Option<u64>`），单 session 几百个 tool
  约 +几 KB——相比 OMIT 砍掉的 ~436 KB 可忽略
- **bytes 估算 vs 真实 token**：4 字符/token 是粗启发式；中文 / 多字节字符比例会偏高
  （UTF-8 中文 3 字节但通常算 1 token），但**所有路径都用同一启发式** = 跳变消失，绝对值
  误差用户视感低
- **回滚**：删除 `apply_tool_output_omit` 内 `output_bytes = match ...` 段即可退回前态；
  字段保留 `None` 不影响其它路径（`#[serde(default)]` 兼容）

## Out of scope

- 真实 token 计数（如调用 tiktoken）——本 change 只解决"展开前后稳定"，精确 token
  计数是更大议题
- 把 OMIT 层 trim 的 size 信息透出给 HTTP 路径——HTTP 当前不走 OMIT，没收益
- 给其他 OMIT 字段（`responses[].content` / `image.data` / `subagent.messages`）加同款
  size 元数据——按需迭代，本 change 只覆盖最痛点

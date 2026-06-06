## Context

`cdt-query` 当前提供两种粒度：原始 chunks（`get_session_detail`）和 session 级聚合（`build_summary`）。缺失 chunk 与聚合之间的「item 级展平」——AI 助手拿到 chunk 级 JSON 后需自行遍历嵌套结构提取目标条目，context token 浪费 >90%。

`sessions errors` 子命令已做了展平（`ErrorEntry`），但实现在 `engine.rs` 且 error message 提取不完善（Bash 工具的 `errorMessage` 常为 `None`，输出 `(no message)`）。

## Goals / Non-Goals

**Goals:**
- 在 `cdt-query` 层新增 `extract` 模块，提供 item 级展平查询
- CLI `--extract` 参数复用现有 filter/window 管道，只在最后一步分叉输出
- 统一 error message 提取逻辑，修复 `sessions errors` 的 `(no message)` 问题
- MCP 可直接复用 extract 模块（本次不改 MCP，留扩展口）

**Non-Goals:**
- 不做嵌套字段投影（`--fields a.b.c`），用 `| jq` 覆盖
- 不做 tool 级二次过滤（`--tool-status error`），`--extract errors` 已覆盖
- 不拆新子命令，`sessions detail` 保持唯一 chunk 级查询入口
- 不改 `cdt-core` 数据模型

## Decisions

### D1: extract 逻辑放 `cdt-query` 层而非 `view.rs`

**候选方案：**
- A. 放 `cdt-cli/src/view.rs`（与现有 `ChunkView` 同层）
- B. 放 `cdt-query/src/extract.rs`（与 `summary.rs` 同层）

**选择 B**。理由：

1. 语义是 query 投影（从 chunks 展平出条目），与 `summary.rs`（从 chunks 聚合统计）同一抽象层级
2. `cdt-cli` 是叶子 crate，不应被其他 crate 依赖；MCP 和未来 HTTP API 可直接复用 `cdt-query`
3. `view.rs` 的职责是格式化（JSON/table），不是数据重组；混入 query 逻辑会让 view 承担投影 + 格式化两件事

**风险：** `extract` 函数需要 `summarize_input`（目前在 `view.rs`）来生成 `input_summary`。解决：将 `summarize_input` 提到 `cdt-query` 层，或在 `extract` 中内联一个轻量版。

### D2: 统一 error message 提取放 `cdt-query`，不放 `cdt-core`

**候选方案：**
- A. 作为 `cdt-core::ToolExecution` 的方法
- B. 作为 `cdt-query::extract` 的内部函数

**选择 B**。理由：

1. `cdt-core` 是数据模型层，不应包含展示策略（"stderr 截取最后 N 行"是策略决策）
2. `extract_errors()` 需要调用此函数填充 `error_summary` 字段，放 `view.rs` 会形成 query → view 反向依赖

提取优先级：
1. `errorMessage` 字段（如有）
2. `ToolOutput::Structured { value }` 时：读取 `value["stderr"]` / `value["error"]` / `value["message"]`；读取 `value["exit_code"]` 或 `value["exitCode"]` 构造 "exit code N"
3. `ToolOutput::Text { text }` 时：regex 匹配 `exit code \d+` 或 `exit status \d+`
4. output 最后 200 字符作为 fallback
5. 以上均无内容时返回 `None`

**codex 审查修正（Finding 2）**：原设计只考虑了 text output 的 regex 匹配，遗漏了 `ToolOutput::Structured` 的一等结构化字段。Bash 工具的 output 可能是 `Structured { value: {"stdout":..., "stderr":..., "exit_code":...} }`，直接 `serde_json::to_string` 后 regex 不可靠。

### D3: `ErrorEntry` 废弃迁移，不保留双轨

**候选方案：**
- A. 保留 `ErrorEntry` + 新增 `ToolExecEntry` 并存
- B. 废弃 `ErrorEntry`，`get_session_errors()` 内部委托 `extract_errors()`

**选择 B**。理由：

`ErrorEntry` 是 `ToolExecEntry` 的严格子集，保留两套 = 两套提取逻辑 + 两套错误格式化路径。`get_session_errors()` 标 `#[deprecated]`，内部实现改为调 `extract_errors()` 后映射回 `ErrorEntry` 结构保持 API 兼容。

### D4: `--extract` 与现有管道的组合语义

数据管道不变，`--extract` 只影响最后的输出转换：

```
fetch all chunks → filter(kind) → grep → range/tail → ┬→ (无 --extract) 原有 ChunkView 路径
                                                       └→ (有 --extract) extract.*() → format
```

`--extract` 的输入是经过 filter/window 后的 `&[(usize, &Chunk)]`（保留绝对索引），与 `--filter` / `--range` / `--tail` / `--grep` 全部正交组合。

**codex 审查修正（Finding 1）**：原设计让 `extract_*` 接收 `&[Chunk]` 并内部 `enumerate()`，会丢失绝对 chunk 索引。现改为接收 `&[(usize, &Chunk)]`，CLI 传入已携带绝对索引的 window 结果，输出的 `chunkIndex` 与 `sessions detail` 不带 `--extract` 时一致。

`--extract` 与 `--content` SHALL 互斥——使用 clap `conflicts_with = "content"` 直接报错，不静默忽略。

**codex 审查修正（Finding 5）**：原设计静默忽略 `--content`，会让用户无效组合变成静默行为。改为显式 clap 冲突报错。

### D5: text 和 JSON 双输出格式

`--extract` 默认输出 text（每行一条，AI 读起来 token 最少）；加 `--format json` 输出扁平 JSON array（脚本可消费）。

text 格式不做终端宽度自适应截断——extract 的目标是结构化数据，不是人类 terminal 阅读体验。长字段（command、file path）截断到固定 80 字符。

### D6: `summarize_input` 复用策略

`view.rs::summarize_input()` 目前在 `cdt-cli` 里。extract 需要类似的 input 摘要能力。

**选择：将 `view.rs::summarize_input()` 下沉到 `cdt-query::extract` 模块，view.rs 改为调用 `cdt-query` 的版本**。

**codex 审查修正（Finding 3）**：原设计计划独立实现两套 `summarize_input`，codex 指出同名字段 `inputSummary` 两套静默分叉会让用户在 `--extract tools` 和 `--content omit` 中看到不同摘要。改为共用同一实现——`summarize_input` 本身是通用的 JSON object → 前 3 key 摘要逻辑，无展示策略成分，放 `cdt-query` 合理。`view.rs` 改为 `pub use cdt_query::extract::summarize_input;`。

## Risks / Trade-offs

- **`get_session_errors()` 废弃影响 MCP**：MCP 的 `get_session_errors` tool 内部调用 `engine.get_session_errors()`。本次先在 engine 层做委托保持兼容，MCP 迁移留后续 change。**codex 审查修正（Finding 4）**：迁移前 SHALL 补 MCP fixture 测试，断言 `get_session_errors` 的 envelope / pagination / camelCase 字段不变
- **text 格式是否足够稳定给脚本消费**：text 格式面向 AI 阅读，不保证跨版本兼容；需要稳定契约的用 `--format json`
- **extract 后 `--json=fields` 投影**：当前 `--json` 的 top-level 投影可以作用于 extract 输出的扁平结构，但这是自然组合而非显式设计，暂不写入 spec

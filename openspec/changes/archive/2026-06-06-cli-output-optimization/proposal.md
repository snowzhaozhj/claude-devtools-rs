## Why

CLI (`cdt`) 的数据访问粒度太粗——最小返回单位是 chunk 级别，一个 AI chunk 可达 1.3MB（137 tool 调用 + 212 responses）。agent 通过 session-insights skill 消费 CLI 输出时，为看 1 条 error message 被迫拿到整个 chunk，导致 ~435K tokens 浪费（理想路径只需 ~1.3K tokens，300× 差距）。同时 CLI 和 MCP 的 view 层分叉严重：MCP 已有 `ContentMode::Omit/Full` + `ChunkEnvelope` + `summarize_input` + 分页，CLI 直接序列化 raw domain objects，没有任何内容控制。此外存在多个格式契约缺陷（jsonl 假行为、空结果 exit(2)、truncate 不一致）。

## What Changes

### 共享 view 层提取
- 从 `mcp/mod.rs` 提取协议无关的 view 组件（`ChunkView`/`ToolExecView`/`ResponseView`/`ContentField`/`ContentMode`/`build_chunk_view()`/`summarize_input()` 等）到共享模块 `crate::view`
- MCP 改为引用共享模块，消除代码分叉

### CLI `sessions detail` 内容控制
- 新增 `--content <omit|full>` flag，与 MCP `content_mode` 对齐
- 不指定时保持原有 raw JSON 输出（兼容）
- `--content omit`：输出 ChunkView 包装，~500B/chunk vs ~200KB/chunk
- grep 命中 chunk auto-expand 为 full（复用 MCP 行为）

### CLI grep/window 语义统一
- 统一 grep 应用顺序为 MCP 语义：kind_filter → grep/context → range/tail
- `--full` 改名 `--all`（原名作 alias），澄清"返回全部 chunk"语义
- `--range` 与 `--tail` 加互斥校验（与 MCP 对齐）

### 格式契约修正
- `sessions summary`/`sessions cost`/`stats` 的 `--format jsonl` 输出紧凑单行 JSON（当前与 json 相同，是 bug）
- 四处空结果 `exit(2)` 改为 `exit(0)`（JSON 输出空值 + exit 0）
- 统一三个 truncate 函数为 unicode-width-aware 的 `truncate_display`

### `--json <fields>` 字段选择（Phase 2）
- 全局 flag，隐含 format=json + 字段过滤 + 紧凑输出
- 无参数时列出可用字段名（类似 gh CLI）

### `--no-truncate` flag（Phase 2）
- 全局 flag，table 模式不截断任何字段

### session-insights skill 更新（Phase 2）
- agent 关键路径加 `--content omit` 或 `--json <fields>`

### table 显示优化（Phase 3）
- PATH `~/` 缩写；unicode-width 中文对齐；terminal-size 弹性列分配

## Capabilities

### New Capabilities
- `cli-output`: CLI 输出格式化、内容控制、字段选择、table 渲染的行为契约

### Modified Capabilities
- `mcp-server`: view 层提取重构——ChunkEnvelope/ToolExecEnvelope/ContentMode 等从 MCP 模块提取到共享 view 层，MCP 功能不变但内部结构调整

## Impact

- **cdt-cli crate**：新增 `view.rs` 模块；`main.rs` 加 `--content`/`--all`/`--json`/`--no-truncate` flags + grep 顺序重构 + 格式契约修正；`mcp/mod.rs` 删除提取出去的 view 定义改为引用共享模块
- **session-insights skill**：更新命令示例
- **新增依赖**：`unicode-width`（Phase 1）、`terminal-size`（Phase 3）
- **不涉及**：cdt-core / cdt-api / cdt-query / cdt-analyze / src-tauri / ui

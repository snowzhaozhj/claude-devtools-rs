## ADDED Requirements

### Requirement: CLI turn 视图输出

`cdt session <id>` SHALL 默认输出 turn 视图（compact overview，与 MCP `get_session` 同形）。系统 SHALL 提供 `cdt turn <id> <n>`（单 turn 完整 steps，同 MCP `get_turn`）与 `cdt step-output <id> <t> <s>`（单 step 完整原文，同 MCP `get_step_output`）子命令。`cdt session <id> --raw` SHALL 保留原 chunk 输出作为调试逃生舱。

CLI 的数据参数 SHALL 与 MCP 工具完全一致；CLI 仅额外提供终端渲染 flags（`--format`、`--json`、`--no-truncate`、`--raw`），这些非数据参数 MCP 不需要。

#### Scenario: session 默认输出 turn 视图

- **WHEN** 运行 `cdt session <id>`
- **THEN** 输出 SHALL 为 turn 列表，每个 turn 含 question、answer、聚合后的工具用量、metrics

#### Scenario: turn 子命令取完整调用链

- **WHEN** 运行 `cdt turn <id> <n>`
- **THEN** 输出 SHALL 为第 n 个 turn 的有序 steps（含 tool 的 input/output），大 output 按阈值截断

#### Scenario: step-output 取完整原文

- **WHEN** 运行 `cdt step-output <id> <t> <s>`
- **THEN** 输出 SHALL 为该 step 的完整未截断 output

#### Scenario: --raw 保留原 chunk 逃生舱

- **WHEN** 运行 `cdt session <id> --raw`
- **THEN** 输出 SHALL 为原始 chunk 结构（非 turn 视图）

## REMOVED Requirements

### Requirement: Content mode 控制 JSON/JSONL 输出粒度

**Reason**: `content_mode`（omit/overview/full）是「为省 token 逼多次调用」的反模式根源；turn 模型 + 服务端内置截断（tool output ≥5KB）+ `get_step_output` 取全文替代之，默认即返回 AI 可直接分析的完整数据。

**Migration**: 用 `cdt session <id>`（默认 turn compact）+ `cdt turn <id> <n>`（完整 steps）+ `cdt step-output`（全文）替代 `--content-mode` 各档；`--raw` 取原 chunk。

### Requirement: range 与 tail 互斥

**Reason**: `range` / `tail` / `cursor` 三选一的 chunk 窗口选择被 turn index 寻址（`cdt turn <id> <n>`）+ 统一 `nextCursor` 分页替代，不再有互斥窗口参数。

**Migration**: 用 turn index 直接定位单 turn；列表分页统一走 `nextCursor`。

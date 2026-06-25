## Context

桌面端 export 能力完整（Markdown / JSON / HTML），由前端 `ui/src/lib/export/` 实现，数据源经 Tauri IPC `get_session_detail_for_export` 拿到保留 tool output 的 SessionDetail。CLI 当前没有 export 子命令——用户只能 `cdt session <id> --format json` 看 raw JSON。

CLI 是 in-process 调用 `LocalDataApi`，数据源与桌面端一致但不经 IPC 裁剪，天然拿到完整 SessionDetail（含 tool output + response content），不需要新增导出专用数据路径。

TS 原版 export（`../claude-devtools/src/renderer/utils/sessionExporter.ts`）只有 428 行、三格式（markdown / json / plaintext）、不依赖 `buildDisplayItems`——直接遍历 chunk 字段。本仓前端在 port 时增强为 HTML + TOC + `buildDisplayItems` 时序合并。CLI 导出参照 TS 原版的简单路径，不走 `buildDisplayItems`。

## Goals / Non-Goals

**Goals:**
- CLI offline 导出会话为 Markdown / JSON
- 管道友好（默认 stdout，-o 写文件）
- 工具输出详略控制（full / summary / name-only）
- thinking / subagent 可选包含
- 与现有 --range / --tail / --grep 过滤参数正交组合

**Non-Goals:**
- HTML 格式导出（需 `pulldown-cmark` + `ammonia`，留 Phase 2）
- `buildDisplayItems` 时序合并（TS 原版 export 也不用；CLI 按 chunk 字段自然序渲染）
- 批量多 session 合并导出
- 前端 export UI 改动
- 桌面端 export 行为变更

## Decisions

### D1：export 模块放 `cdt-cli/src/export.rs`，不建独立 crate

**候选**：
- A. `cdt-cli/src/export.rs` 内部模块
- B. 独立 `cdt-export` crate

**选 A**。理由：当前唯一消费者是 CLI；若未来 Tauri 后端也需要（如 IPC 导出），再抽 crate。函数签名按 crate 边界设计（纯函数接 `&SessionDetail` + options 返 `String`），搬迁成本 < 10 分钟。

### D2：参照 TS 原版直接遍历 chunk 字段，不走 `buildDisplayItems`

**候选**：
- A. 移植 `buildDisplayItems` 到 Rust，与前端 export 时序对齐
- B. 直接遍历 `Chunk::Ai.responses` / `tool_executions` / `semantic_steps`，按自然序渲染

**选 B**。理由：TS 原版 sessionExporter.ts 就是 B 路径且运行多年无投诉。`buildDisplayItems` 处理 subagent 配对、slash、teammate 等复杂逻辑（583 行），CLI MVP 不需要。codex 二审指出"spec 要求复用 buildDisplayItems"——查看 spec 原文，该要求针对的是**桌面端** export（`session-export` spec 的"导出对话流时序" Requirement 上下文是 SessionMetaMenu UI 入口），CLI 作为新入口可按 chunk 自然序渲染，spec delta 会显式声明 CLI 路径的时序契约。

**风险**：AI chunk 内 tool 调用与文本的渲染顺序可能与桌面端不同。→ 缓解：CLI Markdown 按 responses → tool_executions 顺序渲染（先文本后工具），足够可读；用户需要精确时序用桌面端导出。

### D3：CLI 语法选 `cdt export <id>` 独立子命令，不扩展 `cdt session <id> --format md`

**候选**：
- A. `cdt export <id> --format md` 独立子命令
- B. `cdt session <id> --format md` 扩展现有 --format

**选 A**。理由：现有 `--format json/jsonl/table` 是**查看格式**（查数据），export 是**文档生成**（生产物），语义不同。独立子命令 UX 更清晰，不会跟 `--chunks` / `--content` / `--extract` 互斥矩阵冲突。`cdt export` 有自己的 `--format md/json`，与全局 `--format` 隔离。

### D4：JSON 导出走投影后的 SessionDetail，不是 raw pretty-print

**候选**：
- A. `serde_json::to_string_pretty(&detail)` 直接 dump
- B. 按 ExportOptions 投影后再 pretty-print

**选 B**。理由：用户选了 `--no-thinking` 应该在 JSON 里也看不到 thinking step；选了 `--detail name-only` 应该在 JSON 里也没有 tool input/output。与前端 `jsonExporter.ts` 行为一致（前端也走 `projectSessionDetail` 投影后再 `JSON.stringify`）。

### D5：Markdown 元数据表从 `build_summary` + `compute_session_cost` 组装

CLI 的 `SessionDetail` metrics 只有 `message_count`。`build_summary` 提供 `total_duration_ms` / `tool_usage` / `phases` 等，`compute_session_cost` 提供 `total_cost` / `total_tokens` / `model`。`cmd_session_inspect` 已有这两个调用链，export 复用同一模式。

### D6：`--detail` 三档对应 ToolOutputMode

| CLI flag | ToolOutputMode | 行为 |
|---|---|---|
| `--detail full` (默认) | Full | 工具 input + output 完整渲染 |
| `--detail summary` | Truncated | output 截断到 2000 字符 + `... (truncated)` |
| `--detail name-only` | NameOnly | 仅渲染工具名，不含 input/output |

截断用 `.chars().take(n)` 按 Unicode scalar boundary，不是 TS 的 UTF-16 `slice`。

## Risks / Trade-offs

- **两份渲染逻辑维护** → Chunk 结构由 spec 约束变化慢；export 格式不频繁变；MVP < 300 行 Rust 维护负担可控
- **大 session 内存** → CLI 一次性加载完整 SessionDetail + 生成完整 export string；10k 消息 session 约 120MB RSS（bench 基线），export string 再翻倍约 240MB → CLI 单次执行可接受
- **与桌面端 export 不完全对齐** → CLI 不走 `buildDisplayItems`，tool/text 渲染顺序可能微差 → Phase 2 引入 HTML 时可考虑统一，MVP 可接受

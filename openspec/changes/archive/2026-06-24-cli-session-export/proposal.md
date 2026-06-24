## Why

桌面端已有 Markdown / JSON / HTML 三格式会话导出（`ui/src/lib/export/`），但 CLI (`cdt`) 缺少对应能力。用户在无 GUI 场景（SSH、CI、脚本批处理）下需要将特定会话导出为可读文档（回顾学习、分享协作、决策存档），目前只能 `cdt session <id> --format json` 拿原始 JSON 手工翻阅。CLI 导出是 offline 场景（不需要 server 在跑），且 Rust 侧 `Chunk` 类型 + `view.rs` 辅助函数已是半成品，增量实现成本低。

## What Changes

- 新增 `cdt export <session-id>` 顶级子命令，默认输出到 stdout，支持 `-o <path>` 写文件
- 支持 `--format md` (默认) / `--format json` 两种格式；HTML 留 Phase 2（需 `pulldown-cmark` + `ammonia` 依赖）
- 支持 `--detail full` (默认) / `--detail summary` / `--detail name-only` 三档工具输出详略，对应前端 `ExportOptions.toolOutputMode`
- 支持 `--no-thinking` 排除 thinking blocks（默认包含）
- 支持 `--no-subagents` 排除子代理卡片（默认包含）
- 复用现有 `--range` / `--tail` / `--grep` / `--filter` 参数过滤 chunk 范围
- 新增 `crates/cdt-cli/src/export.rs` 模块，复用 `view.rs` 的 `message_content_text` / `tool_output_text` + `cdt-query` 的 `build_summary` / `compute_session_cost`
- Markdown 导出结构对齐前端：`# 标题` + 元数据表 + `## Turn N — {Role}` 分段 + 工具调用三级标题
- JSON 导出为 `SessionDetail` pretty-print，受 `--detail` / `--no-thinking` / `--no-subagents` 投影影响

## Capabilities

### New Capabilities
<!-- 无新增 capability -->

### Modified Capabilities
- `session-export`: 新增 CLI 导出路径 Requirement——CLI `cdt export` SHALL 支持 Markdown / JSON 两种格式，与桌面端导出共享相同的内容结构契约（元数据表、turn 结构、工具输出详略控制）
- `cli-output`: 新增 `cdt export` 子命令定义——命令语法、参数、输出行为、与现有过滤参数的组合语义

## Impact

- **后端**：`crates/cdt-cli/src/export.rs`（新文件，~300 行）；`crates/cdt-cli/src/main.rs`（新增 `Export` 子命令 + `cmd_export` 入口）
- **依赖**：零新增 crate 依赖（Markdown 纯字符串拼接；JSON 已有 `serde_json`）
- **IPC 协议**：无变更——CLI 直接调用 `LocalDataApi::get_session_detail` in-process，不经 IPC/HTTP
- **性能**：CLI 导出是一次性用户动作，全量加载 SessionDetail 后渲染，无热路径影响

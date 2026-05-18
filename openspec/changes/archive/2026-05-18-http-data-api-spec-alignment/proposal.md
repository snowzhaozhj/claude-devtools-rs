## Why

`openspec/specs/http-data-api/spec.md` 当前对路由 URL 与错误码描述与 Rust 实现 (`crates/cdt-api/src/http/routes.rs`) 存在多处偏差：搜索端点 URL 错配（spec 写 `POST /api/search/sessions`，实现 `POST /api/search`）；spec 仅枚举 5 个端点，实际实现 25+ 路由；错误码段只列了 `validation_error/400`、`not_found/404`、`internal/500` 三种，实际实现还有 `config_error/400` 与 `ssh_error/502` 两个未写入。`openspec/followups.md::http-data-api [spec-gap] 路由前缀与错误码全部与实现偏差` 已点明本 gap，且该 followup 是若干其他 http followup（SSE ssh-status / updater 事件源、http session detail CLAUDE.md 测试）的前提——主 spec 不对齐，无法精确表达后续 followup 的差量。

## What Changes

- 主 spec `Serve projects and sessions over HTTP under /api prefix` Requirement 在保持既有规约句的前提下补齐项目/会话域全部实际路由（`POST /api/projects/{projectId}/session-summaries/batch` 已被列为缺失项；`POST /api/sessions/batch` 已部分写）。
- 主 spec `Serve search endpoints` Requirement URL 修正为 `POST /api/search`（与实现一致），保留 body schema 同形约束。
- 主 spec `Serve auxiliary, subagent, utility, and validation endpoints` Requirement 补齐 SSH/contexts/path validation/CLAUDE.md/mentioned-file/agent-configs/repository-groups/worktrees 等辅助路由清单。
- 主 spec `Serve config and notification endpoints` Requirement 把通知系列 5 个路由 + `PATCH /api/config` URL 落到具体 endpoint。
- 主 spec `Push events via Server-Sent Events` Requirement 显式写明 SSE 端点路径 `GET /api/events`。
- 主 spec `Return safe defaults on lookup failures (current baseline)` Requirement 扩为完整状态码表：补 `code: "config_error"` + `400`、`code: "ssh_error"` + `502 Bad Gateway` 两个映射；保留既有 `validation_error/400`、`not_found/404`、`internal/500` 三条；保留与 TS 基线对比段。
- 不改任何代码、不改前端、不改 IPC 契约。仅 spec 与实现描述对齐。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `http-data-api`: 路由 URL 清单与错误码 `code` × HTTP status 映射两处与实现对齐。

## Impact

- 影响文件：`openspec/specs/http-data-api/spec.md`（archive 时由 `openspec archive` 自动 sync）。
- 不影响代码：`crates/cdt-api/src/http/routes.rs`、`crates/cdt-api/src/ipc/error.rs` 行为不变；现有 `tests/http_*.rs`、`api_error_*_maps_to_*` 单测继续通过。
- 解锁后续 followup：SSE `ssh-status` / `updater` 事件源接通（`coverage-gap` 已在 followups.md）、`http_session_detail` CLAUDE.md 测试 fixture 加固、若干 http 路由文档化。本 change 完成后这些 followup 的 spec delta 才能精确表达增量。

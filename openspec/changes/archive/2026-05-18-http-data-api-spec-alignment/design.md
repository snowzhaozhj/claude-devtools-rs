## Context

主 spec 是行为契约真相源；archive 时由 `openspec archive` 自动 sync 回 `openspec/specs/<cap>/spec.md`。当前 `http-data-api/spec.md` 大部分 Requirement 文字描述与 Rust 实现 (`crates/cdt-api/src/http/routes.rs`) 不一致：

- 路由层面：spec 写 `POST /api/search/sessions`，实现是 `POST /api/search`；spec 仅列项目/会话/搜索/通知/SSE 五段端点中的代表性几个，实现额外有 SSH/contexts/path validation/CLAUDE.md/agent-configs/repository-groups/worktrees 等共 25+ 路由。
- 错误码层面：spec `Return safe defaults on lookup failures (current baseline)` 段只提 `validation_error/400`、`not_found/404`、`internal/500` 三种；`crates/cdt-api/src/ipc/error.rs::ApiErrorCode` 还有 `ConfigError → 400`、`SshError → 502 Bad Gateway`，且 `routes.rs::IntoResponse for ApiError` 已实现这些映射。

`openspec/followups.md::http-data-api [spec-gap] 路由前缀与错误码全部与实现偏差` 把"改 spec 把前缀写成 /api，把路由形态贴近实现"列为决策，但实际并未执行。多条 followup（SSE 增量补 ssh-status / updater；http session detail CLAUDE.md fixture）都依赖主 spec 已对齐。

## Goals / Non-Goals

**Goals:**
- 主 spec 文字与实现 100% 对齐：endpoint URL、HTTP 方法、错误码 `code` × HTTP status 映射。
- 保留既有 Requirement 的核心规约句（`find_session_project` 反查 / batch 缺失返 `not_found` 占位 / SSE producer broadcast 桥接 / port 占用 fail-fast 等），仅扩列表与状态码。
- followups.md 同步更新：把该条 [spec-gap] 标 ✅ 已修复并指向本 change。

**Non-Goals:**
- 不改 `crates/cdt-api/src/http/routes.rs`、`crates/cdt-api/src/ipc/error.rs` 等代码；现有行为已经符合即将更新的 spec。
- 不接通 SSE `ssh-status` / `updater` 事件源——是独立 followup（依赖本 change 完成后再开新 change）。
- 不补 http session detail CLAUDE.md fixture——独立 followup。
- 不调整路由 URL 风格（`{projectId}` vs `:projectId`）；spec 与实现一致用 `{...}` 占位即可。
- 不引入 OpenAPI 规范文档；本 change 只在 prose Markdown 内列路由清单。

## Decisions

### D1: spec 用"完整路由清单"形式 vs"代表性 Scenario + 必要约束句"形式

候选：

- **方案 A（采纳）**：在每个 Requirement body 内追加完整路由清单（`HTTP方法 + URL + 用途` 一行一条），保留既有 Scenario 仅作典型行为示例。
- **方案 B**：每个路由都拆一条 `#### Scenario:`，完全枚举。
- **方案 C**：拆出独立 `http-routes` capability。

理由（采纳 A）：

- B 把 25+ 路由各拆一条 Scenario，spec 文件膨胀 200+ 行，对 reviewer 阅读价值低（routing 是机械映射，行为本质在 IPC 层）；测试粒度也跟不上——不会真的为每条路由写一个端到端 HTTP 测试。
- C 是 followup 提到的可选项，但 routing 与 IPC operation 是 1:1 委托，独立 capability 反而把"http 路由形态"从 IPC behavior 上下文里割裂；现阶段保持单 capability。
- A 是 followups.md 决策原话"把路由形态贴近实现"的最直接落地：既给 reviewer 一份可对照实现的清单，又不冲淡 SHALL 句的可测性。

### D2: 错误码段格式——表格 vs 列表

候选：

- **方案 A（采纳）**：用 prose + 列表枚举 `code` 字符串、HTTP status、触发条件三元组。
- **方案 B**：Markdown 表格。

理由：本仓主 spec 全部用 prose + 列表（grep `openspec/specs/*/spec.md` 无表格），保持风格一致。表格 raw markdown 在 git diff 中阅读体验差。

### D3: SshError → 502 Bad Gateway 的语义

候选：

- **方案 A（采纳）**：spec 显式写 `code: "ssh_error"` + `502 Bad Gateway`，触发条件为"远端 SSH 连接 / 命令执行失败（超时、握手失败、远端命令非零退出等）"。
- **方案 B**：合并到 `internal/500`。

理由：`502 Bad Gateway` 在 HTTP 语义里就是"上游网关错误"，对 SSH backend 是天然映射；合并到 500 会丢失"远端不可达 vs 本地 panic"的语义区分，对调用方（远端浏览器客户端）排错有价值。这个映射是实现已经做了的事，spec 跟随实现。

### D4: ConfigError → 400 的归类

候选：

- **方案 A（采纳）**：与 `validation_error` 并列 400，但 `code` 字段独立为 `"config_error"`，触发条件为"配置 JSON 文件解析失败 / 配置字段值非法"。
- **方案 B**：合并到 `validation_error`。

理由：`code` 字符串是给客户端做错误分支的，"用户输入校验失败"和"持久化配置损坏"在 UI 上需要不同提示文案；HTTP status 都是 400 但 `code` 区分。实现已分两个 enum variant，spec 跟随。

### D5: 是否在 spec 中冻结 SSE 端点路径 `GET /api/events`

候选：

- **方案 A（采纳）**：写明 `GET /api/events` 是 SSE 端点路径。
- **方案 B**：仅约束"暴露一个 SSE endpoint"不绑定路径。

理由：本仓约定是"主 spec 接近行为契约 + 关键 URL 形状有意义"，前端 / 远端客户端按此路径 reconnect。冻结路径让 reviewer 看 spec 即可知道该订哪个 URL，不需要再去读 `routes.rs`。

## Risks / Trade-offs

- **Risk**：路由清单与代码可能未来再次漂移（新加 endpoint 没回写 spec）。
  → Mitigation：在 tasks.md 的 archive 后跟进里加一条"任何新加 http endpoint 的 PR 都 SHALL 同步在本 Requirement 路由清单加一行"，并记入 `crates/CLAUDE.md::IPC vs HTTP 行为分叉` 段（如已存在则放过）。本 change 不强制做这步基础设施，仅在 followups.md 留 hint。

- **Risk**：spec 写了 `GET /api/events` 路径但未来若需重命名（如改为 `/api/sse`）会触发 BREAKING。
  → Mitigation：远端浏览器/SSE 客户端是仓外依赖，已经按 `/api/events` 在用；改路径本来就是 BREAKING，spec 显式冻结路径反而强制下次改名时走 propose 流程，是良性约束。

- **Trade-off**：本 change 只对齐文字、不改代码、不新增测试。表面上"零代码风险"，但同时也意味着 codex 二审能查的范围有限——主要审 spec delta 文字逻辑严密性、是否漏 SHALL 句、与现有 Scenario 是否兼容；不是性能 / 并发类深度审。这是 spec-only change 的固有 ROI 上限。

## Migration Plan

无破坏性变更：spec 文字调整向后兼容（HTTP 客户端的实际行为不变）。

archive 后：
1. `openspec/followups.md::http-data-api [spec-gap] 路由前缀与错误码全部与实现偏差` 标 ✅ 并补"指向 change `http-data-api-spec-alignment`"。
2. 该 [spec-gap] 解锁后，可独立开 change `http-sse-ssh-status-and-updater` 接通 followup 第二条 SSE 事件源；以及 change `http-session-detail-claudemd-fixture` 加固 fixture。这些是后续 PR，不在本 change 范围。

## Open Questions

无。所有决策由"实现现状 + followups.md 明文决策"已锁定。

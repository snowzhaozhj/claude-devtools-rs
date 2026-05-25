# openspec-slim-M-tier

## Why

8 个 M 档（中体量、反引号密度高）capability 的主 spec 含大量"实现选择 SHALL 句"、源码路径、内部 fn / type / mod 名、IPC contract test 级 scenario，违反 `change spec-purity-mechanism` 的反模式。它们让 spec 成为"代码实现说明书"而非"行为契约真相源"，提高 reviewer 心智负担、阻碍未来重构。

本 change 仅做**主 spec 瘦身**：把实现细节移出主 spec（必要时去 design.md `## Decisions`），把 IPC contract test 级 scenario 下沉为测试存在性 tasks，合并颗粒过细的"每工具/每按键/每命令一个"scenario 为"行为类别 + 白名单常量"。

**不**做 capability 边界重构（边界整合走 GitHub Issue #296 单独评估）。

## What Changes

8 个 capability 主 spec 同步瘦身（每个走 MODIFIED Requirement delta，行为契约 SHALL/MUST 句**保留**，IPC 字段名 / 字段语义**不动**）：

- `configuration-management`（600 → 期望 < 400 行）：剔除 `serde(default = "<fn>")` 之类内部 attribute 描述、合并 `externalEditor / terminalApp / searchEngine` 三个枚举字段的"每枚举值一个 scenario"、删 `update_config` IPC handler 内部分流注释级 SHALL。
- `fs-abstraction`（549 → < 350）：删 H1-H6 enforce 机制内的 xtask 路径 / `cargo test -p` 命令 / 集成测试文件名 / 12 方法 trait 的内部 attribute 链；保留 7 个核心方法 + 3 个写方法的行为契约。
- `tool-execution-linking`（322 → < 250）：把"工具块右键菜单"两条 Requirement 收敛到 `frontend-context-menu` 的"按 surface 分 factory"框架引用；保留 `Pair tool_use with tool_result by id` 等核心算法 SHALL。
- `chunk-building`（302 → < 230）：`Embed teammate messages into AIChunk` 的 5 步实现细节移 design（保 4 个 Scenario 的可观察行为契约）；删 `EMBED_TEAMMATES=false` 回滚开关 Scenario（实现层 toggle 不属契约）。
- `project-discovery`（439 → < 320）：合并 4 个 Windows 路径解析 Scenario 为 1 个"WSL / HOME fallback 链"；剔除 `cdt-discover::path_decoder` / `worktree_grouper.rs::78-117` 等源码引用；保留 `Scan / Decode / Group / Encode` 行为契约。
- `http-data-api`（392 → < 270）：合并三组路由"实际路由清单"（项目/会话/搜索/通知/lazy）为单段"路由清单是 IPC 镜像 + camelCase 一致"行为契约 + 路由表移 design；保留 SSE / CORS / static fallback / `sse_lagged` sentinel 行为契约。
- `keyboard-shortcuts`（449 → < 300）：合并"normalizeBindingToMod token 算法 11 步规则"为"幂等 / 跨平台一致 / 保留辅助修饰键"行为契约；删 ID / 路径 / 数量等内部清单细节；保留 14 内置快捷键白名单矩阵。
- `frontend-context-menu`（516 → < 320）：合并 8 个 factory function 名 + 4 处 Layer1/2/3 内部分层细节为"右键菜单分层 + 按 surface 分 factory"行为契约；保留全局兜底 / WKWebView smart-select 防护 / submenu 视觉 / IPC 契约（`open_in_terminal` / `open_in_editor`）。

每条主 spec 的 SHALL / MUST 行为句逐条审计——保留行为不变，仅把"实现选择 / 源码路径 / 内部 fn 名 / 测试级 scenario"标记 MODIFIED 替换。

## Impact

- Affected specs（MODIFIED-only，无 ADDED / REMOVED Requirement）：configuration-management, fs-abstraction, tool-execution-linking, chunk-building, project-discovery, http-data-api, keyboard-shortcuts, frontend-context-menu
- Affected code: 0（仅 spec 文档重写）
- 风险：行为句被误删 → codex design 二审 + reviewer + spec-fidelity-reviewer subagent 三道审查覆盖
- 不影响 capability 边界（边界重构走 Issue #296）
- 不影响 IPC 字段名 / 字段语义 / 测试代码 / 后端实现

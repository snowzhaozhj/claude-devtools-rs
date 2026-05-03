## ADDED Requirements

### Requirement: Expose git branch on session summary and metadata updates

`SessionSummary` 与 `SessionMetadataUpdate` SHALL 在已有字段集（`sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing`）之外**额外**携带 `git_branch: Option<String>` 字段（IPC 序列化时为 camelCase `gitBranch`）。骨架返回（`list_sessions` 同步阶段）SHALL 为 `None`，真实值由后端异步元数据扫描在 `LocalDataApi::list_sessions` 后台 JoinSet 任务内填充并通过 `session-metadata-update` 事件 push 到前端。

后端取值规则：解析 session JSONL 时 SHALL 遍历 `cdt_parse::ParsedMessage.message.git_branch`，记录**最后一条** `Some(...)` 作为最终值（与原版 `claude-devtools/src/renderer/utils/sessionExporter.ts` 取值方式一致——反映会话最后所在的 git 分支）。session 中所有行的 `git_branch` 都为 `None`（非 git 仓库）时 SHALL 保持 `None`。

`cdt-api/tests/ipc_contract.rs` SHALL 加断言验证 `SessionSummary` 与 `SessionMetadataUpdate` 序列化结果含 `gitBranch` camelCase 字段，与 `messageCount` 等同位。

#### Scenario: list_sessions skeleton has gitBranch null

- **WHEN** caller 调用 `list_sessions("p")`
- **THEN** 同步返回的每个 `SessionSummary` SHALL 含字段 `gitBranch`（值为 `null`，因尚未异步扫描）

#### Scenario: session-metadata-update payload contains gitBranch

- **WHEN** 后端后台扫描某个 session 完毕，最后一行 `git_branch` 为 `Some("feat/foo")`
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload SHALL 含 `gitBranch: "feat/foo"`（camelCase）

#### Scenario: session without any git_branch line

- **WHEN** 后端扫描 session 所有行 `git_branch` 均为 `None`（非 git 项目）
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload `gitBranch` SHALL 为 `null`

#### Scenario: backend takes last non-empty git_branch

- **WHEN** session 内消息行 `git_branch` 序列依次为 `Some("main")` / `None` / `Some("feat/x")` / `Some("feat/y")` / `None`
- **THEN** 该 session 元数据推送的 `gitBranch` SHALL 为 `"feat/y"`（最后一条非空）

#### Scenario: contract test asserts camelCase serialization

- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** 断言 `SessionSummary { git_branch: Some("main"), ... }` 序列化为 JSON 后 SHALL 含字段名 `"gitBranch"`，且 `SessionMetadataUpdate` 同样

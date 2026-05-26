## Why

`ipc-data-api` 已膨胀至 56 Requirement / 290 Scenario / 2518 行，成为全仓最大 spec（超第二名 2×）。大量 Requirement 描述的是**领域业务逻辑**（SSH 操作、teammate 元数据、仓库分组、缓存策略、push event 广播），而非 IPC 协议层关注点。这违背 SPEC_GUIDE "single owner" 原则——同一行为在 domain cap 和 ipc-data-api 双重描述，改一处漏另一处。issue #296 计划 + issue #303 PR 9 执行位。

## What Changes

- 从 `ipc-data-api` 迁出 ~30 个 Requirement（含 ~180 Scenario）到 7 个 domain capability
- `ipc-data-api` 收敛到 ~26 个 Requirement，仅描述 IPC 协议层：序列化契约、payload 瘦身（OMIT 系列）、lazy load 协议、pagination 协议、Tauri command 注册、校验/错误码、ProjectScanCache/Invalidator 基础设施
- 行为契约 100% 不变（字符级搬运，不修订 SHALL/WHEN/THEN 子句）
- 不改代码 / 测试 / 配置 / IPC 字段名（纯 spec 文档重组）

## Capabilities

### New Capabilities

（无新建 capability——所有迁移目标已存在）

### Modified Capabilities

- `ipc-data-api`: REMOVED ~31 个越界 Requirement（SSH / teammate / chunk / project-discovery / push-events / cache / telemetry / server-mode / memory 领域）
- `ssh-remote-context`: ADDED 1 Requirement（Expose SSH and context operations）
- `team-coordination-metadata`: ADDED 4 Requirement（teammate messages / spawn metadata / tags strip / subagent count）
- `chunk-building`: ADDED 2 Requirement（CompactChunk derived metadata / Stable chunk identifiers）
- `project-discovery`: ADDED 8 Requirement（git branch / repo groups / worktree sessions / Tauri commands / resolve project id / group listing / Tauri group cmd / worktree 元信息）
- `server-mode`: ADDED 1 Requirement（http_server_start / _stop / _status commands）
- `session-parsing`: ADDED 9 Requirement（metadata cache × 3 / parsed-message cache × 3 / title × 3）
- `push-events`: ADDED 3 Requirement（file-change + notifications push / stream detected errors / session metadata updates）
- `application-telemetry`: ADDED 2 Requirement（telemetry snapshot / correctness event batch）
- `memory-viewer`: ADDED 1 Requirement（Expose memory read operations）

## Impact

- 纯 spec 文档改动，不影响 Rust 代码 / 前端 / IPC 协议 / 测试
- 8 个现有 spec 对 `ipc-data-api` 的引用需检查是否仍正确（描述性引用保持不变，不需改）
- `spec-fidelity-reviewer` 的 Scenario→test 映射不受影响（Requirement 标题不变，只换 owner cap）

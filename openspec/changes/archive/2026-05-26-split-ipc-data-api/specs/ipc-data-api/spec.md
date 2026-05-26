# ipc-data-api Specification (delta)

## REMOVED Requirements

### Requirement: Expose SSH and context operations

**Reason**：本 Requirement 拆出到 `ssh-remote-context` capability。

**Migration**：行为契约 100% 不变；由 `ssh-remote-context` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Emit push events for file changes and notifications

**Reason**：本 Requirement 拆出到 `push-events` capability。

**Migration**：行为契约 100% 不变；由 `push-events` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Stream detected errors to subscribers

**Reason**：本 Requirement 拆出到 `push-events` capability。

**Migration**：行为契约 100% 不变；由 `push-events` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Emit session metadata updates

**Reason**：本 Requirement 拆出到 `push-events` capability。

**Migration**：行为契约 100% 不变；由 `push-events` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose teammate messages on AIChunk

**Reason**：本 Requirement 拆出到 `team-coordination-metadata` capability。

**Migration**：行为契约 100% 不变；由 `team-coordination-metadata` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose teammate spawn metadata on ToolExecution

**Reason**：本 Requirement 拆出到 `team-coordination-metadata` capability。

**Migration**：行为契约 100% 不变；由 `team-coordination-metadata` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Strip teammate-message tags from session title

**Reason**：本 Requirement 拆出到 `team-coordination-metadata` capability。

**Migration**：行为契约 100% 不变；由 `team-coordination-metadata` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose subagent messages total count

**Reason**：本 Requirement 拆出到 `team-coordination-metadata` capability。

**Migration**：行为契约 100% 不变；由 `team-coordination-metadata` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Resolve project id from session id alone

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose git branch on session summary and metadata updates

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose CompactChunk derived metadata in SessionDetail

**Reason**：本 Requirement 拆出到 `chunk-building` capability。

**Migration**：行为契约 100% 不变；由 `chunk-building` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose repository group queries

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose worktree sessions query

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Tauri commands for repository groups and worktree sessions

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: `extract_session_metadata` 按 `FileSignature` 缓存

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose memory read operations

**Reason**：本 Requirement 拆出到 `memory-viewer` capability。

**Migration**：行为契约 100% 不变；由 `memory-viewer` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: `extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: parsed-message 缓存按 file-change 广播主动失效

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Stable chunk identifiers in SessionDetail

**Reason**：本 Requirement 拆出到 `chunk-building` capability。

**Migration**：行为契约 100% 不变；由 `chunk-building` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Title length is bounded by TITLE_MAX_CHARS constant

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Title algorithm changes do not invalidate MetadataCache

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: IPC SHALL expose http_server_start / _stop / _status commands

**Reason**：本 Requirement 拆出到 `server-mode` capability。

**Migration**：行为契约 100% 不变；由 `server-mode` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose group session listing via k-way merge pagination

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Tauri command for list_group_sessions

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: SessionSummary 增加 worktree 元信息字段

**Reason**：本 Requirement 拆出到 `project-discovery` capability。

**Migration**：行为契约 100% 不变；由 `project-discovery` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose telemetry snapshot pull endpoint

**Reason**：本 Requirement 拆出到 `application-telemetry` capability。

**Migration**：行为契约 100% 不变；由 `application-telemetry` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: Expose telemetry correctness event batch endpoint

**Reason**：本 Requirement 拆出到 `application-telemetry` capability。

**Migration**：行为契约 100% 不变；由 `application-telemetry` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

### Requirement: SessionDetail 暴露与 SessionSummary 同源派生的 title

**Reason**：本 Requirement 拆出到 `session-parsing` capability。

**Migration**：行为契约 100% 不变；由 `session-parsing` capability 内同名 Requirement 守护。所有 Scenario 字符级迁移到新 cap。

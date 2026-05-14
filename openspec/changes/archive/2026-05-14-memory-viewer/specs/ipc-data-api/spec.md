## ADDED Requirements

### Requirement: Expose memory read operations

系统 SHALL 暴露只读 memory IPC 操作，允许前端按项目查询 memory layers 与读取单个 memory 文件。响应字段 MUST 使用 camelCase，且新 Tauri command MUST 同步登记到 command contract 与前端 mock command 清单。

#### Scenario: Query project memory via IPC
- **WHEN** 前端调用 `invoke("get_project_memory", { projectId: "p" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`hasMemory`、`count`、`defaultFile`、`layers` 字段
- **AND** `layers` 内每项 SHALL 含 `file`、`title`、`hook`、`kind` 字段（camelCase）

#### Scenario: Read memory file via IPC
- **WHEN** 前端调用 `invoke("read_memory_file", { projectId: "p", file: "MEMORY.md" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`file`、`filePath`、`content` 字段

#### Scenario: Tauri commands registered
- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** `EXPECTED_TAURI_COMMANDS` SHALL 包含 `get_project_memory` 与 `read_memory_file`

#### Scenario: Memory IPC camelCase serialization
- **WHEN** Rust 侧 `ProjectMemory` 与 `MemoryLayer` 被序列化为 JSON
- **THEN** 字段名 SHALL 为 `hasMemory`、`defaultFile`、`projectId`，而不是 snake_case

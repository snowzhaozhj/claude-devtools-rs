# memory-viewer Specification (delta)

## ADDED Requirements

### Requirement: Expose memory read operations

系统 SHALL 暴露 memory IPC 操作，覆盖 layers 查询、单文件读取、文件新增 / 覆盖、文件删除四类。响应字段 MUST 使用 camelCase，且新 Tauri command MUST 同步登记到 command contract 与前端 mock command 清单。

写路径 IPC（`add_memory` / `delete_memory`）SHALL 在写入完成后内部调 `discover_memory_layers` 拿最新 layers 状态，直接返新的 `ProjectMemory` payload（前端无需再调 `get_project_memory` 二次查询，避免 IPC 重入开销）。

写路径 SHALL 复用 `validate_memory_file_name` 校验函数与 read 路径同语义——拒绝路径穿越 / 绝对路径 / 非 `.md` 文件 / 含 `/` `\` `:` 字符的文件名。校验失败 SHALL 返 `ApiError::validation` 且不触发任何 fs 写操作。

`add_memory` 在目标文件已存在时 SHALL 走 atomic 覆盖语义（tmp + rename），`delete_memory` 在目标文件不存在时 SHALL 返 `ApiError::not_found`。

#### Scenario: Query project memory via IPC
- **WHEN** 前端调用 `invoke("get_project_memory", { projectId: "p" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`hasMemory`、`count`、`defaultFile`、`layers` 字段
- **AND** `layers` 内每项 SHALL 含 `file`、`title`、`hook`、`kind` 字段（camelCase）

#### Scenario: Read memory file via IPC
- **WHEN** 前端调用 `invoke("read_memory_file", { projectId: "p", file: "MEMORY.md" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`file`、`filePath`、`content` 字段

#### Scenario: Add memory file via IPC
- **WHEN** 前端调用 `invoke("add_memory", { projectId: "p", file: "feedback_test.md", content: "..." })`
- **THEN** 系统 SHALL atomic 写入文件到该项目 `memory/` 目录（不存在时自动 `create_dir_all`）
- **AND** 响应 SHALL 为 JSON object 形如 `ProjectMemory`（含 `projectId`、`hasMemory: true`、`count`、`defaultFile`、`layers` 字段）反映写入后的状态
- **AND** 文件已存在时 SHALL 覆盖（atomic write tmp+rename 语义）

#### Scenario: Delete memory file via IPC
- **WHEN** 前端调用 `invoke("delete_memory", { projectId: "p", file: "feedback_test.md" })` 且文件存在
- **THEN** 系统 SHALL 调 `fs.remove_file` 删除该文件
- **AND** 响应 SHALL 为 JSON object 形如 `ProjectMemory` 反映删除后的状态
- **AND** 删除最后一个 `.md` 文件后 `hasMemory` SHALL 为 `false` 且 `layers` SHALL 为空

#### Scenario: Memory write path validates file name
- **WHEN** 前端调用 `invoke("add_memory", { projectId: "p", file: "../secret.md", content: "..." })` 或 `file: "secret.json"` 或 `file: "subdir/note.md"`
- **THEN** 响应 SHALL 为 `ApiError::validation`，文案与既有 `read_memory_file` 校验失败一致
- **AND** SHALL NOT 触发任何 fs 写操作

#### Scenario: Delete missing memory file returns not_found
- **WHEN** 前端调用 `invoke("delete_memory", { projectId: "p", file: "ghost.md" })` 且文件不存在
- **THEN** 响应 SHALL 为 `ApiError::not_found`
- **AND** SHALL NOT 影响 memory 目录其他文件

#### Scenario: Tauri commands registered
- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** `EXPECTED_TAURI_COMMANDS` SHALL 包含 `get_project_memory`、`read_memory_file`、`add_memory`、`delete_memory` 四项

#### Scenario: Memory IPC camelCase serialization
- **WHEN** Rust 侧 `ProjectMemory` 与 `MemoryLayer` 被序列化为 JSON
- **THEN** 字段名 SHALL 为 `hasMemory`、`defaultFile`、`projectId`，而不是 snake_case


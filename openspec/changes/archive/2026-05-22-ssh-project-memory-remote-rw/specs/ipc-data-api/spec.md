## MODIFIED Requirements

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

### Requirement: Dispatch project/session reads by active context

所有"读项目 / 读会话 / 读会话产物 / 全局搜索 / 项目 memory CRUD"类 IPC method 在 active context = `Ssh<host>` 时 SHALL 走当前 SSH `FileSystemProvider`（通过 `LocalDataApi::active_scanner()` 或 `LocalDataApi::active_fs_and_projects_dir()` helper），**不得**直接锁 `self.scanner` / `self.projects_dir` 字段而退化到本地数据。本 Requirement 覆盖的 method 集合 SHALL 至少包含以下 13 个：

**本 change 修复（8 处）**：
- `list_repository_groups`
- `project_memory_dir`
- `find_session_project`
- `get_session_summaries_by_ids`
- `get_subagent_trace`
- `get_image_asset`
- `get_tool_output`
- `search`

**已正确实现（3 处，本 change 加回归测试）**：
- `list_sessions` / `list_sessions_sync` / `list_sessions_paginated`
- `get_session_detail`
- `list_projects`

**memory 读写（4 处，change `ssh-project-memory-remote-rw` 起 SHALL 走 active SSH provider，不再 graceful skip）**：
- `get_project_memory` —— 走 `fs.read_dir` + `fs.read_to_string` 读远端 memory 目录
- `read_memory_file` —— 走 `fs.read_to_string` 读远端 memory 文件
- `add_memory` —— 走 `fs.create_dir_all` + `fs.write_atomic` 写远端 memory 文件，写完调 `discover_memory_layers` 返新 ProjectMemory
- `delete_memory` —— 走 `fs.remove_file` 删远端 memory 文件，删完调 `discover_memory_layers` 返新 ProjectMemory

**例外**：仅"重置本地数据根路径"语义的 method（`set_projects_dir` / `reconfigure_claude_root`）保持 local provider，不受本条约束。

#### Scenario: list_repository_groups 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_repository_groups` IPC
- **THEN** 系统 SHALL 通过当前 SSH context 的 `FileSystemProvider` 扫描 `<remote_home>/.claude/projects/`
- **AND** 返回的 `RepositoryGroup.worktrees[]` SHALL 来自远端 fixture 的项目集合
- **AND** 返回结果 SHALL NOT 包含本地宿主机 `.git` 解析出的 `gitBranch` 值

#### Scenario: 辅助读类 method 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `find_session_project(session_id)` / `project_memory_dir(project_id)` / `get_session_summaries_by_ids(ids)` 任一
- **THEN** 后端 SHALL 通过当前 SSH context 的 provider 读远端文件
- **AND** 返回的 project_id / path 字段 SHALL 与远端 fake fixture 一致
- **AND** 返回的路径字段（若存在，如 `project_memory_dir`）SHALL 以远端 `<remote_home>` 为根

#### Scenario: 会话产物读取在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `get_subagent_trace(session_id, ...)` / `get_image_asset(session_id, ...)` / `get_tool_output(session_id, ...)` 任一
- **THEN** 后端 SHALL 通过远端 SFTP 读取对应文件
- **AND** 远端 provider 的 `read_file` 调用计数 SHALL ≥ 1（fake provider 通过 `Mutex<usize>` 计数器观测）
- **AND** 本地 `LocalFileSystemProvider` 的同名方法 SHALL NOT 被调用

#### Scenario: search 在 SSH context 下使用 active provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `search(query)` IPC
- **THEN** `SessionSearcher` SHALL 接收当前 SSH provider 作为 `Arc<dyn FileSystemProvider>` 入参
- **AND** 搜索结果 SHALL 来自远端 `<remote_home>/.claude/projects/` 下的 jsonl 内容
- **AND** 后端**不得**硬编码 `LocalFileSystemProvider::new()` 作为 search 的数据源
- **AND** 远端 provider 的 `read_to_string` 或 `open_read_stream` 调用计数 SHALL ≥ 1

#### Scenario: 根路径重置类 method 仍用 local provider

- **WHEN** 调用方调 `set_projects_dir(new_path)` 或 `reconfigure_claude_root(new_root)`
- **THEN** 系统 SHALL 重置 `self.scanner` 为 `LocalFileSystemProvider` 包装下的新 `projects_dir`
- **AND** 该重置**不影响**已注册的 SSH context 的 provider 状态
- **AND** 若 active context 是 SSH，**仍**保持 SSH 为 active；后续调"读项目/会话"类 method 仍走 SSH provider
- **AND** 仅当 active context 切回 local 后，新的 local `projects_dir` 才生效

#### Scenario: 已实现的 method 在 SSH context 下保持远端行为

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_projects` / `list_sessions` / `list_sessions_sync` / `list_sessions_paginated` / `get_session_detail` 任一
- **THEN** 后端 SHALL 走 SSH provider 读远端数据（行为与本 change 前一致）
- **AND** 本 Requirement 配套的回归测试 SHALL 覆盖这 5 个 method，防止后续改动误退化为 local

#### Scenario: memory CRUD 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `get_project_memory(project_id)` / `read_memory_file(project_id, file)` / `add_memory(project_id, file, content)` / `delete_memory(project_id, file)` 任一
- **THEN** 后端 SHALL 通过当前 SSH context 的 fs provider 调对应远端 fs ops（read_dir / read_to_string / write_atomic / create_dir_all / remove_file）
- **AND** 远端 fake provider 的对应 op counter（`read_dir_count` / `read_count` / `write_count` / `mkdir_count` / `rename_count` / `remove_count`）SHALL ≥ 1
- **AND** 本地 `LocalFileSystemProvider` 的同名方法 SHALL NOT 被调用
- **AND** 旧的 graceful skip 行为（`has_memory: false` / not_found 含 "SSH context" 字样）SHALL NOT 出现

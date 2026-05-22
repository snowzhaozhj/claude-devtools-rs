## ADDED Requirements

### Requirement: Operate memory CRUD over current backend

系统 SHALL 在当前 active context（`Local` 或 `Ssh<host>`）下提供项目 memory 的完整 CRUD 行为契约：layer 发现、单文件读取、单文件新增 / 覆盖、单文件删除四类操作 SHALL 通过 `cdt-fs::FileSystemProvider` trait 调用 backend-specific fs ops，不感知具体 backend 类型。

`add_memory` / `delete_memory` SHALL 在写入 / 删除完成后内部调 `discover_memory_layers` 收集最新 layers，与新的 `ProjectMemory` payload 一同返回——这让前端 UI 状态（如 `Sidebar.memoryCache`）可直接 swap 到最新 `ProjectMemory` 而无需二次调用 `get_project_memory`。

`add_memory` 在目标项目 memory 目录不存在时 SHALL 通过 `fs.create_dir_all` 自动创建后再写入文件——用户首次为某项目添加 memory 不应因目录缺失而失败。

写路径 SHALL 复用 `validate_memory_file_name` 校验函数与 read 路径同语义；校验失败 SHALL 返 `ApiError::validation` 且不触发任何 fs 写操作。

写路径 SHALL 是 atomic：reader 永远观察到旧内容或新内容整版，不观察到截断 / 半写状态；具体实现走 `fs.write_atomic` trait 方法（详 fs-abstraction spec `Requirement: FileSystemProvider trait 暴露 7 个核心方法` 写方法 atomic 契约段）。

`memory-viewer` UI（`ui/src/lib/views/MemoryView.svelte`）当前不规约 add / delete 按钮——本 Requirement 只规约 IPC 行为契约；UI 加按钮接入 add/delete 路径属于后续 follow-up，不在本 capability 当前 spec 范围内。

#### Scenario: SSH context 下 layers 发现走远端

- **WHEN** active context 是 `Ssh<host>` 且当前项目远端 memory 目录含 `MEMORY.md` + `feedback_chinese_language.md`
- **THEN** UI 调 `get_project_memory(project_id)` SHALL 拿到 `hasMemory: true` + 含 index layer + entry layer 的 `ProjectMemory`
- **AND** 行为与 Local context 等价——UI 渲染路径 SHALL NOT 因 backend 类型不同而走分叉

#### Scenario: SSH context 下读单文件走远端

- **WHEN** active context 是 `Ssh<host>` 且远端 memory 目录含 `MEMORY.md`
- **THEN** UI 调 `read_memory_file(project_id, "MEMORY.md")` SHALL 拿到远端文件内容
- **AND** 返回的 `filePath` SHALL 以远端 `<remote_home>` 为根

#### Scenario: add_memory 写入并返新 ProjectMemory

- **WHEN** UI 调 `add_memory(project_id, "feedback_test.md", "content body")`
- **THEN** 后端 SHALL atomic 写入文件到 `<memory_dir>/feedback_test.md`
- **AND** 响应 SHALL 是新的 `ProjectMemory`，`layers` 中含 `feedback_test.md` 作为 orphan（如未被 MEMORY.md 索引）或 entry（如被索引）
- **AND** 前端 SHALL 能直接 swap 状态，不需再调 `get_project_memory`

#### Scenario: add_memory 在 memory 目录缺失时自动创建

- **WHEN** UI 调 `add_memory(project_id, "first_note.md", "...")` 且该项目 `memory/` 目录尚不存在
- **THEN** 后端 SHALL 调 `fs.create_dir_all(<memory_dir>)` 创建目录
- **AND** SHALL 继续 atomic 写入文件
- **AND** 返回的 `ProjectMemory.hasMemory` SHALL 为 `true`

#### Scenario: delete_memory 删除并返新 ProjectMemory

- **WHEN** UI 调 `delete_memory(project_id, "feedback_test.md")` 且该文件存在
- **THEN** 后端 SHALL 调 `fs.remove_file` 删除文件
- **AND** 响应 SHALL 是新的 `ProjectMemory`，`layers` 不再包含该文件
- **AND** 删除最后一个 `.md` 后 `hasMemory` SHALL 为 `false`

#### Scenario: 写路径文件名校验拒绝路径穿越

- **WHEN** UI 调 `add_memory(project_id, "../etc/passwd", "...")` 或 `add_memory(project_id, "secret.json", "...")` 或 `delete_memory(project_id, "subdir/note.md")`
- **THEN** 响应 SHALL 是 `ApiError::validation`
- **AND** SHALL NOT 触发任何 fs 写 / 删操作

#### Scenario: 写路径 atomic（reader 不观察半写）

- **WHEN** caller A 调 `add_memory(project_id, "MEMORY.md", "<new content>")`，与此同时 caller B 并发调 `read_memory_file(project_id, "MEMORY.md")`
- **THEN** caller B 的响应 SHALL 是旧内容整版或新内容整版，绝不返回截断 / 半写中间态
- **AND** atomic 保证由 fs trait `write_atomic` 实现层（tmp + rename）提供

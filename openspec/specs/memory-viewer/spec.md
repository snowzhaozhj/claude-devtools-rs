# memory-viewer Specification

## Purpose
TBD - created by archiving change memory-viewer. Update Purpose after archive.
## Requirements
### Requirement: Discover project memory layers

系统 SHALL 对指定项目发现 `memory/` 目录中的 Markdown memory 文件，并返回结构化 layers 列表供 UI 展示。`MEMORY.md` SHALL 作为固定 index layer；`MEMORY.md` 中引用的文件 SHALL 作为 entry layer；目录中未被索引引用的 `.md` 文件 SHALL 作为 orphan layer。系统 MUST 只列出 `.md` 文件，且不得返回目录外路径。

#### Scenario: 项目含 MEMORY.md 与条目文件
- **WHEN** 项目 memory 目录含 `MEMORY.md`，其中索引引用 `feedback_chinese_language.md`
- **AND** 该文件存在于同一 memory 目录
- **THEN** 返回 layers SHALL 包含一个 `kind = "index"` 的 `MEMORY.md` layer 和一个 `kind = "entry"` 的 `feedback_chinese_language.md` layer

#### Scenario: 未被索引引用的 Markdown 文件作为 orphan
- **WHEN** 项目 memory 目录含 `MEMORY.md` 与 `extra_note.md`
- **AND** `MEMORY.md` 未引用 `extra_note.md`
- **THEN** 返回 layers SHALL 包含 `extra_note.md`，其 `kind` SHALL 为 `"orphan"`

#### Scenario: 无 memory 目录时返回空状态
- **WHEN** 指定项目没有 `memory/` 目录
- **THEN** 系统 SHALL 返回 `hasMemory = false`、`count = 0`、`layers = []`，不报错

### Requirement: Read a memory file safely

系统 SHALL 支持读取指定项目 memory 目录内的单个 Markdown 文件内容。读取接口 MUST 拒绝目录穿越、绝对路径、非 `.md` 文件和目录项；读取失败时 SHALL 返回结构化错误而不是 panic。

#### Scenario: 读取 index 文件
- **WHEN** 调用方请求读取 `MEMORY.md`
- **THEN** 响应 SHALL 返回该文件完整 Markdown 内容和文件名 `MEMORY.md`

#### Scenario: 拒绝目录穿越
- **WHEN** 调用方请求读取 `../settings.json`
- **THEN** 系统 SHALL 拒绝该请求，且 SHALL NOT 读取 memory 目录外的任何文件

#### Scenario: 拒绝非 Markdown 文件
- **WHEN** 调用方请求读取 `secret.json`
- **THEN** 系统 SHALL 返回 validation error，且 SHALL NOT 返回文件内容

### Requirement: Render memory view

Memory 页面 SHALL 以 master/detail 布局展示 memory layers：左侧显示 layers 列表，右侧渲染当前选中文件的 Markdown 内容。首次打开时 SHALL 默认选中 `MEMORY.md`；若没有 `MEMORY.md`，则选中第一个 layer；若无 layers，则展示空状态。右侧 Markdown 内指向同目录 `.md` memory 文件的链接 MUST 在应用内切换选中文件，SHALL NOT 触发 webview 页面导航。

#### Scenario: 首次打开默认选中 MEMORY.md
- **WHEN** 用户打开一个含 `MEMORY.md` 的 Memory tab
- **THEN** 左侧 SHALL 高亮 `MEMORY.md` layer，右侧 SHALL 渲染 `MEMORY.md` 内容

#### Scenario: 点击 layer 切换预览
- **WHEN** 用户点击左侧 `feedback_chinese_language.md` layer
- **THEN** 右侧 SHALL 读取并渲染该文件内容，左侧 SHALL 高亮该 layer

#### Scenario: 点击 Markdown 内部 memory 链接切换预览
- **WHEN** 用户在 `MEMORY.md` 预览内点击 `[始终使用中文](feedback_chinese_language.md)` 链接
- **THEN** 当前选中文件 SHALL 切换为 `feedback_chinese_language.md`，右侧内容 SHALL 更新
- **AND** 浏览器 URL SHALL 保持不变，webview SHALL NOT 整页导航或刷新

#### Scenario: 空 memory 状态
- **WHEN** Memory tab 加载结果 `layers = []`
- **THEN** 页面 SHALL 显示无 memory 的空状态，而不是崩溃或显示空白

### Requirement: Operate on selected memory file

Memory 页面 SHALL 提供对当前选中文件的操作：文件下拉 SHALL 切换当前选中文件；Open SHALL 通过系统默认应用打开当前 Markdown 文件；Copy SHALL 把当前 Markdown 原文写入系统剪贴板并展示短暂成功或失败反馈。若当前没有选中文件或文件内容尚未加载完成，文件级操作 SHALL 禁用或不执行。

#### Scenario: 下拉切换当前文件
- **WHEN** 用户在文件下拉中选择 `feedback_chinese_language.md`
- **THEN** 右侧 SHALL 读取并渲染该文件内容，左侧 SHALL 高亮该 layer

#### Scenario: 打开当前文件
- **WHEN** 用户选中 `MEMORY.md` 并点击 Open
- **THEN** 系统 SHALL 通过系统默认应用打开该 `MEMORY.md` 文件路径

#### Scenario: 复制当前 Markdown 原文
- **WHEN** 用户选中 `MEMORY.md` 并点击 Copy
- **THEN** 系统 SHALL 将该 `MEMORY.md` 的 Markdown 原文写入剪贴板

#### Scenario: 无选中文件时禁用文件操作
- **WHEN** Memory tab 没有任何选中文件或文件内容尚未加载完成
- **THEN** Open 与 Copy 操作 SHALL 禁用或不执行

### Requirement: Operate memory CRUD over current backend

系统 SHALL 在当前 active context（`Local` 或 `Ssh<host>`）下提供项目 memory 的完整 CRUD 行为契约：layer 发现、单文件读取、单文件新增 / 覆盖、单文件删除四类操作 SHALL 通过统一的文件系统抽象层调用 backend-specific fs ops，调用方代码不感知具体 backend 类型。

`add_memory` / `delete_memory` SHALL 在写入 / 删除完成后内部重新发现 layers，与新的 `ProjectMemory` payload 一同返回——这让前端 UI 状态可直接 swap 到最新 `ProjectMemory` 而无需二次调用 `get_project_memory`。

`add_memory` 在目标项目 memory 目录不存在时 SHALL 自动创建目录后再写入文件——用户首次为某项目添加 memory 不应因目录缺失而失败。

写路径 SHALL 复用与读路径同语义的文件名校验；校验失败 SHALL 返 `ApiError::validation` 且不触发任何 fs 写操作。

写路径 SHALL 是 atomic：reader 永远观察到旧内容或新内容整版，不观察到截断 / 半写状态；具体由文件系统抽象层（`fs-abstraction` capability `Requirement: FileSystemProvider trait 暴露 7 个核心方法` 的 atomic 写约束）保证。

UI 层"新增 / 删除按钮"接入路径不在本 capability 范围内——本 Requirement 只规约 IPC 行为契约。

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
- **THEN** 后端 SHALL 自动创建 memory 目录
- **AND** SHALL 继续 atomic 写入文件
- **AND** 返回的 `ProjectMemory.hasMemory` SHALL 为 `true`

#### Scenario: delete_memory 删除并返新 ProjectMemory

- **WHEN** UI 调 `delete_memory(project_id, "feedback_test.md")` 且该文件存在
- **THEN** 后端 SHALL 删除文件
- **AND** 响应 SHALL 是新的 `ProjectMemory`，`layers` 不再包含该文件
- **AND** 删除最后一个 `.md` 后 `hasMemory` SHALL 为 `false`

#### Scenario: 写路径文件名校验拒绝路径穿越

- **WHEN** UI 调 `add_memory(project_id, "../etc/passwd", "...")` 或 `add_memory(project_id, "secret.json", "...")` 或 `delete_memory(project_id, "subdir/note.md")`
- **THEN** 响应 SHALL 是 `ApiError::validation`
- **AND** SHALL NOT 触发任何 fs 写 / 删操作

#### Scenario: 写路径 atomic（reader 不观察半写）

- **WHEN** caller A 调 `add_memory(project_id, "MEMORY.md", "<new content>")`，与此同时 caller B 并发调 `read_memory_file(project_id, "MEMORY.md")`
- **THEN** caller B 的响应 SHALL 是旧内容整版或新内容整版，绝不返回截断 / 半写中间态
- **AND** atomic 保证由文件系统抽象层提供


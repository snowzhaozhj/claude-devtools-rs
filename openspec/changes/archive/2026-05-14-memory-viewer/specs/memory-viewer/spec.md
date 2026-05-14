## ADDED Requirements

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

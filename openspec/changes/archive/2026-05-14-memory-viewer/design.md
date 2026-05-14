## Context

Rust 端口当前只有 session/settings/notifications tab，Sidebar 也只展示项目与会话。原版 claude-devtools 已有 project-scoped Memory 入口：读取 `~/.claude/projects/<encoded-project>/memory/` 下的 `MEMORY.md` 和关联 `.md` 文件，左侧列出 layers，右侧渲染 Markdown。用户截图要求 Rust 端口至少支持 Memory 入口、layers 列表、Markdown 预览、下拉切换和复制内容。

## Goals / Non-Goals

**Goals:**

- 在 Sidebar 中为有 memory 的项目显示 `Memory (N)` 入口。
- 打开单例 Memory tab，并在 tab 内展示当前项目的 memory layers。
- 通过 Tauri IPC 只读读取 memory index、memory 文件列表和单文件内容。
- 前端复用现有 Markdown 渲染能力，支持文件切换和复制当前内容。
- 保持 IPC 字段 camelCase，并补 contract/mock/test 覆盖。

**Non-Goals:**

- 不提供 memory 创建、编辑、删除或重命名。
- 不实现 memory 文件系统实时监听；本 change 只在打开/刷新时读取。
- 不支持跨项目汇总 memory；Memory tab 绑定单个项目。
- 不在本 change 中实现原版 `openIn` / `copyPath` 的系统文件管理器操作，除非现有 opener 基础设施可直接复用且不扩大协议面。

## Decisions

### D1: Memory 数据模型以 project-scoped IPC 为边界

新增 `get_project_memory(project_id)` 和 `read_memory_file(project_id, file)` 两类只读 IPC。`get_project_memory` 返回目录是否存在、layers 列表、默认文件和 index 原文；`read_memory_file` 只返回选中文件内容。

候选方案：一次性返回所有文件内容。拒绝原因是 memory 文件数量可能增长，且 UI 首屏只需要列表和默认文件；按需读取能控制 IPC payload，并与原版 `readFile` 行为一致。

### D2: 后端只允许读取 memory 目录内的 `.md` 文件

后端根据 project id 定位 `~/.claude/projects/<project_id>/memory/`，列出 `.md` 文件，读取时规范化请求文件名并拒绝 `..`、绝对路径、非 `.md` 后缀和目录穿越。

候选方案：前端传绝对路径。拒绝原因是 Tauri IPC 属于系统边界，不能信任前端路径；由后端限定根目录更安全，也更容易测试。

### D3: `MEMORY.md` 解析在后端完成，前端只消费结构化 layers

后端解析 `MEMORY.md` 中的条目，返回 `MemoryLayer { file, title, hook, kind }`。`MEMORY.md` 固定作为 `index` layer；索引中引用的文件作为 `entry` layer；目录中未被索引引用的 `.md` 文件作为 `orphan` layer。

候选方案：前端解析 Markdown。拒绝原因是 Rust contract test 更适合覆盖索引解析、orphan 排序和路径过滤；前端保持展示层职责。

### D4: Memory tab 是 project-scoped singleton

`TabType` 增加 `memory`，tab 带 `projectId`。同一项目重复点击 Sidebar Memory 入口时复用已有 Memory tab；不同项目可以各自打开一个 Memory tab。

候选方案：全局唯一 Memory tab，切换项目时替换内容。拒绝原因是用户可能同时比较多个项目的 memory；project-scoped singleton 与 session tab 语义更一致。

### D5: UI 先对齐截图的核心能力，链接跳转作为同页增强

Memory 页面采用左侧 layers + 右侧 Markdown 预览；顶部提供 Copy 按钮和文件下拉。Markdown 内同目录 `.md` 链接和 `[[wikilink]]` 若实现成本低，则解析为切换当前文件；否则先渲染为普通文本/链接，不阻断核心功能。

候选方案：一次性完整移植原版 frontmatter 卡片、open folder、wikilink resolver。拒绝原因是本 change 的首要目标是查看能力可用；系统打开文件夹属于额外协议面，frontmatter 视觉细节不影响查看主路径。

## Risks / Trade-offs

- [Risk] `MEMORY.md` 项目符号格式可能与原版 memoryIndex parser 细节不完全一致 → Mitigation：对照原版 `parseMemoryIndex` 移植核心规则，并用截图中的索引格式加测试。
- [Risk] memory 目录在不同 project id 编码下找不到 → Mitigation：复用 `cdt_discover::path_decoder` / 现有 project discovery 的 encoded project id 规则，不新增私有编码实现。
- [Risk] 复制按钮依赖浏览器 clipboard 权限 → Mitigation：优先使用 `navigator.clipboard.writeText`，失败时显示错误，不影响查看。
- [Risk] 新 tab 类型遗漏某处 switch 导致空白页 → Mitigation：同步 `TabType`、`PaneView`、`TabBar`、mock/test，并用 UI check/e2e 覆盖。

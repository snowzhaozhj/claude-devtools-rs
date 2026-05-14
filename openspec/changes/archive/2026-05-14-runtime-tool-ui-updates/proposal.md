## Why

用户在真实使用中遇到多处运行时体验问题：新增项目不会自动出现在项目入口中，工具详情的可读性、性能和错误反馈也不完整。这些问题影响 Claude Code 会话监控的即时性与排障效率，需要在同一轮迭代中修复数据刷新、工具渲染与 subagent 明细展示的行为契约。

## What Changes

- 项目发现与导航界面在 `~/.claude/projects/` 新增项目目录或新增会话文件后自动刷新，无需重启应用或手动重新加载。
- Edit 工具详情改进 diff 预览与语法高亮，确保 old/new 内容可读并能按文件类型高亮。
- subagent 执行链中的工具明细展示时间统计，让用户能看到工具耗时或仍在等待结果。
- 优化工具调用结果展开路径，避免展开大型输出时同步渲染造成明显卡顿。
- 工具调用失败时展示失败原因，包含 raw error 文本或结构化错误内容，而不是只显示失败状态。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- `file-watching`: 新增项目目录与新会话文件需要触发可被 UI 消费的刷新事件。
- `project-discovery`: 项目列表在运行时变化后需要重新扫描并暴露新增项目。
- `session-display`: 工具详情、Edit diff、高亮、展开性能、失败原因、subagent 工具时间统计属于会话详情渲染行为。
- `tool-execution-linking`: 工具执行记录需要稳定暴露失败原因与可展示的时间统计来源。

## Impact

- Rust：`cdt-watch`、`cdt-discover`、`cdt-analyze`、`cdt-core`、`cdt-api` 中项目刷新事件、工具执行数据结构与 IPC 序列化可能受影响。
- Tauri：`src-tauri/src/lib.rs` 的 file-change bridge 可能需要传递新增项目/项目列表刷新事件。
- UI：Sidebar、Dashboard、项目选择入口、SessionDetail、Tool Viewer、DiffViewer、SubagentCard、lazy markdown/highlight 渲染路径会受影响。
- 测试：需要补 Rust 单测/IPC contract、Vitest 或 Playwright user story，覆盖新增项目自动刷新、工具错误展示、diff 高亮和工具展开性能路径。

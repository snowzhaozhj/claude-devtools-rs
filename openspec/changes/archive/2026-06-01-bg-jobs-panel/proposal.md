## Why

`claude --bg` 后台任务是高级用户的核心工作流，但目前只能通过 CLI `claude agents` 查看状态。桌面应用缺乏等价 GUI 入口，用户无法在 devtools 里感知后台任务进度、跳转 session、或响应需要输入的阻塞任务。Phase 1 提供完整的只读面板 + badge 通知 + session 跳转。

## What Changes

- 新增 Background Jobs 面板（tab 级视图），展示 `~/.claude/jobs/*/state.json` 的实时状态
- FileWatcher 扩展：监听 jobs 目录变更，事件驱动推送（非轮询）
- TitleBar badge：红(failed) > 黄(blocked) > 绿(ready-for-review)，working/空不显示
- 分组对齐 `claude agents` 原生语义：Ready for review / Needs input / Working / Completed
- Session 跳转：从 job 直接打开对应 session（跨项目可行）
- 降级策略：jobs/ 不存在时零 UI 暴露，不建目录、不 watch

## Capabilities

### New Capabilities
- `background-jobs`: 后台任务面板的数据模型、状态映射、分组逻辑、badge 计算、session 跳转、降级策略

### Modified Capabilities
- `file-watching`: 新增 jobs_dir 监听 + route_event 过滤 state.json + is_dir guard
- `ipc-data-api`: 新增 list_jobs command + broadcast 推送
- `tab-management`: 新增 jobs tab 类型 + 单例语义
- `push-events`: 新增 jobs-update 事件通道

## Impact

- `crates/cdt-core/`：新增 BackgroundJob / JobState / JobSummary 类型
- `crates/cdt-watch/`：FileWatcher 扩展 jobs_dir 监听
- `crates/cdt-api/`：list_jobs IPC + WatcherRuntimeChannels 扩展
- `src-tauri/`：command wrapper + invoke_handler 注册
- `ui/src/`：JobsView.svelte + TitleBar badge + tab 路由
- 依赖：无新外部依赖

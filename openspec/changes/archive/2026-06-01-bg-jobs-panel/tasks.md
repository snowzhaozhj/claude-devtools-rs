## 1. 后端类型定义

- [x] 1.1 cdt-core: BackgroundJob / JobState / JobChild / JobSummary 类型（serde camelCase + default 容错）
- [x] 1.2 cdt-core: JobChangeEvent 类型 + projectId 提取辅助函数

## 2. FileWatcher 扩展

- [x] 2.1 cdt-watch: FileWatcher 加 jobs_dir 字段 + jobs_tx broadcast channel(32)
- [x] 2.2 cdt-watch: start() 加 is_dir() guard + watcher.watch(&jobs_dir, Recursive)
- [x] 2.3 cdt-watch: route_event 加 jobs 分支（严格过滤 components==2 + state.json）
- [x] 2.4 cdt-watch: subscribe_jobs() 公开方法
- [x] 2.5 cdt-watch: 单元测试（route_event jobs 过滤 + subscribe 收发）

## 3. IPC 实现

- [x] 3.1 cdt-api: list_jobs 实现（扫 jobs_dir + parse + FileSignature cache + 分组排序）
- [x] 3.2 cdt-api: WatcherRuntimeChannels 扩展 jobs broadcast + bridge task
- [x] 3.3 cdt-api: reconfigure_claude_root 同步 jobs_dir
- [x] 3.4 src-tauri: list_jobs command wrapper + invoke_handler 注册
- [x] 3.5 src-tauri: emit "jobs-update" + SSE bridge
- [x] 3.6 IPC contract test（list_jobs 字段名 + camelCase 验证）
- [x] 3.7 单元测试（解析容错 / 分组逻辑 / badge 计算 / projectId 提取）

## 4. 前端实现

- [x] 4.1 TabType 加 "jobs" + openJobsTab() 单例 + PaneView 路由
- [x] 4.2 UnifiedTitleBar: jobs icon-btn + badge（红>黄>绿优先级）
- [x] 4.3 JobsView.svelte: 分组列表 + 行结构（indicator/name/detail/PR chip/age/chevron）
- [x] 4.4 JobRow.svelte: 展开详情（intent/metadata/操作按钮）
- [x] 4.5 Session 跳转（openSessionTab + projectId 提取）+ PR 跳浏览器
- [x] 4.6 降级处理（隐藏/空态/error/SSH）
- [x] 4.7 选中态（tonal lift + left indicator）
- [x] 4.8 jobs-update 事件监听 + 自动刷新
- [x] 4.9 ⌘K 命令面板 "Open Jobs" 注册

## 5. 测试

- [x] 5.1 vitest: badge 计算 + 分组逻辑 + state→color 映射
- [x] 5.2 vitest: projectId 提取（linkScanPath / fallback cwd / 均空）
- [x] 5.3 Playwright: 打开 tab / 分组显示 / 展开 / 跳转 / 空态 / 降级

## 6. Job 删除功能（D8）

- [x] 6.1 cdt-api trait: `delete_job` + `delete_completed_jobs` 方法定义
- [x] 6.2 cdt-api LocalDataApi: 实现（调 `claude rm <short_id>`）
- [x] 6.3 src-tauri: 注册 `delete_job` + `delete_completed_jobs` IPC command
- [x] 6.4 ipc_contract: EXPECTED_TAURI_COMMANDS 更新
- [x] 6.5 前端 jobsStore: `deleteJob()` + `deleteCompletedJobs()` 函数
- [x] 6.6 前端 tauriMock: mock handler
- [x] 6.7 前端 JobRow: terminal 状态 hover 显示 × dismiss 按钮
- [x] 6.8 前端 JobsView: Completed group header 显示 "Clear" 按钮

## 7. 视觉层级优化（D9 + D10）

- [x] 7.1 JobRow: completed+有PR → 正常 opacity；completed+无PR → opacity 0.55
- [x] 7.2 JobRow: 行 padding 10px→7px + detail line-height 1.4→1.3
- [x] 7.3 JobRow: focus-within 辅助 × 按钮可达性

## 8. 视觉验收

- [x] 8.1 浅色主题全状态截图 + DESIGN.md Named Rules 检查
- [x] 8.2 深色主题全状态截图 + impeccable critique

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过
- [x] N.4 archive change

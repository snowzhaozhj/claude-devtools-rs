## 1. 后端类型定义

- [ ] 1.1 cdt-core: BackgroundJob / JobState / JobChild / JobSummary 类型（serde camelCase + default 容错）
- [ ] 1.2 cdt-core: JobChangeEvent 类型 + projectId 提取辅助函数

## 2. FileWatcher 扩展

- [ ] 2.1 cdt-watch: FileWatcher 加 jobs_dir 字段 + jobs_tx broadcast channel(32)
- [ ] 2.2 cdt-watch: start() 加 is_dir() guard + watcher.watch(&jobs_dir, Recursive)
- [ ] 2.3 cdt-watch: route_event 加 jobs 分支（严格过滤 components==2 + state.json）
- [ ] 2.4 cdt-watch: subscribe_jobs() 公开方法
- [ ] 2.5 cdt-watch: 单元测试（route_event jobs 过滤 + subscribe 收发）

## 3. IPC 实现

- [ ] 3.1 cdt-api: list_jobs 实现（扫 jobs_dir + parse + FileSignature cache + 分组排序）
- [ ] 3.2 cdt-api: WatcherRuntimeChannels 扩展 jobs broadcast + bridge task
- [ ] 3.3 cdt-api: reconfigure_claude_root 同步 jobs_dir
- [ ] 3.4 src-tauri: list_jobs command wrapper + invoke_handler 注册
- [ ] 3.5 src-tauri: emit "jobs-update" + SSE bridge
- [ ] 3.6 IPC contract test（list_jobs 字段名 + camelCase 验证）
- [ ] 3.7 单元测试（解析容错 / 分组逻辑 / badge 计算 / projectId 提取）

## 4. 前端实现

- [ ] 4.1 TabType 加 "jobs" + openJobsTab() 单例 + PaneView 路由
- [ ] 4.2 UnifiedTitleBar: jobs icon-btn + badge（红>黄>绿优先级）
- [ ] 4.3 JobsView.svelte: 分组列表 + 行结构（indicator/name/detail/PR chip/age/chevron）
- [ ] 4.4 JobRow.svelte: 展开详情（intent/metadata/操作按钮）
- [ ] 4.5 Session 跳转（openSessionTab + projectId 提取）+ PR 跳浏览器
- [ ] 4.6 降级处理（隐藏/空态/error/SSH）
- [ ] 4.7 选中态（tonal lift + left indicator）
- [ ] 4.8 jobs-update 事件监听 + 自动刷新
- [ ] 4.9 ⌘K 命令面板 "Open Jobs" 注册

## 5. 测试

- [ ] 5.1 vitest: badge 计算 + 分组逻辑 + state→color 映射
- [ ] 5.2 vitest: projectId 提取（linkScanPath / fallback cwd / 均空）
- [ ] 5.3 Playwright: 打开 tab / 分组显示 / 展开 / 跳转 / 空态 / 降级

## 6. 视觉验收

- [ ] 6.1 浅色主题全状态截图 + DESIGN.md Named Rules 检查
- [ ] 6.2 深色主题全状态截图 + impeccable critique

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

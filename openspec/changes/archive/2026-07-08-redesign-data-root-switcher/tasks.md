## 1. 前端 root switch 状态边界

- [x] 1.1 在 tab store 增加工作区重置能力：关闭 session / memory tabs，回到单空 pane / Dashboard，并清理 tab session cache 与 tab UI state
- [x] 1.2 在 session list store 增加生产可用的缓存清理入口，root switch 后不复用旧 root 会话列表
- [x] 1.3 在 project data store 增加 root switch reload / clear 语义，防止旧 in-flight project 请求结果污染新 root UI
- [x] 1.4 在 App 层实现 root switch coordinator：配置保存成功后清 selected group、重置工作区、清 root-scoped caches、只刷新一次当前 root project data
- [x] 1.5 让 Sidebar 的 memory cache 随 root switch 清理或重建，避免旧 root Memory 入口状态复用

## 2. Settings 数据目录 UI

- [x] 2.1 重写 Settings → General → 数据目录块：当前路径只显示一次，右侧显示“默认”或“自定义”低权重状态
- [x] 2.2 将“最近使用”从下拉改为轻量列表：过滤当前目录，过滤后为空则隐藏整段，每行仅显示路径与“切换”动作
- [x] 2.3 实现“输入路径”原地编辑：按钮行替换为输入框 + 应用 + 取消，Enter 应用、Esc 取消，展开不造成最近列表跳变
- [x] 2.4 实现保存中、保存失败与 validation error 反馈：失败时保留输入行，不关闭 tabs、不刷新工作台
- [x] 2.5 仅在存在 session / memory tab 时显示“切换会关闭当前会话 tab，并回到工作台”的提示
- [x] 2.6 保持数据目录文案只说明 projects / todos，不暗示 background jobs 会随数据根切换

## 3. 切换流程集成

- [x] 3.1 选择目录、最近列表切换、输入路径应用、恢复默认四条路径统一走同一 root switch 成功处理流程
- [x] 3.2 确保 update_config 成功前不清旧上下文；成功后才关闭 root-scoped tabs 并回 Dashboard
- [x] 3.3 确保切换到无项目数据根时 Dashboard / Sidebar 显示无项目状态，而不是旧项目列表
- [x] 3.4 确保新旧 root 存在相同 group / project id 时，不从旧 root session list / memory cache hydrate

## 4. 测试与验证

- [x] 4.1 更新 `SettingsView.dataRoot.test.svelte.ts`：覆盖当前路径展示、最近列表过滤当前项、最近为空隐藏、输入路径原地展开与错误态
- [x] 4.2 新增/更新 tab store 单测：root switch reset 后 session / memory tabs 被关闭，Dashboard 空 pane 可显示，tab cache/UI state 清理
- [x] 4.3 新增/更新 project/session store 单测：root switch 后旧 in-flight 结果不覆盖新 root，session list cache 不跨 root 复用
- [x] 4.4 增加 App/Settings 集成测试：root 切换成功后只刷新一次 project data，selected group 清理并重新选择当前 root 项目
- [x] 4.5 浏览器视觉验证 Settings 数据目录默认、自定义、输入展开、错误、最近为空/非空状态；截图自检无跳变、无重复当前项、无空白控件
- [x] 4.6 跑 `pnpm --dir ui run check` 与受影响 Vitest
- [x] 4.7 跑 `openspec validate redesign-data-root-switcher --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

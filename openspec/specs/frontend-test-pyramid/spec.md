# frontend-test-pyramid Specification

## Purpose

定义前端测试基础设施的四层金字塔：Rust IPC contract test 守护字段形状、Vitest 单测覆盖纯函数与 store、Playwright 跑 user story 级浏览器集成测试、`mockIPC + Vite dev server` 提供 dev/test 环境的假后端。各层职责互斥不重叠，配合 production bundle 的 mockIPC DCE 校验，让 UI 改动可以在不开 Tauri 窗口的浏览器环境下完成大部分回归。
## Requirements
### Requirement: 测试金字塔分四层且职责互斥

系统 SHALL 通过四层测试基础设施守护前端质量，每层职责互斥不重叠：（1）IPC mock 层提供 dev/test 环境的假后端；（2）E2E 集成测试层跑 user story 级浏览器集成测试；（3）单元测试层跑纯函数和 store 单元测试；（4）IPC 契约测试层守护 IPC 字段形状契约。任何一层都 MUST 不被其他层替代。

#### Scenario: 改 UI 组件触发 E2E 而非单元测试层

- **WHEN** 维护者修改 Sidebar 等渲染组件的交互
- **THEN** 回归覆盖 SHALL 由 E2E 集成测试层的 user story 用例提供，**不**由单元测试层组件单测提供
- **AND** 单元测试层 MUST 不包含针对 dumb 渲染组件（BaseItem / StatusDot / OutputBlock 等）的 mount + assertion 测试

#### Scenario: 改 IPC command 字段触发契约测试而非 E2E

- **WHEN** 维护者修改某 IPC command 返回结构体的字段
- **THEN** IPC 契约测试层 SHALL 至少有一个断言因字段名/形状变化而失败
- **AND** 该层失败 MUST 优先于 mock fixture 同步——fixture 漂移是次生问题，契约测试是首要守护

#### Scenario: 加纯算法函数触发单元测试而非 E2E

- **WHEN** 维护者新增一个纯函数（无 DOM、无 IPC 依赖，如格式化时长 / 解析 URL）
- **THEN** 该函数 SHALL 由同名单元测试文件覆盖
- **AND** E2E 用例 MUST 不为验证此类纯函数而存在

### Requirement: mockIPC 必须覆盖所有 Tauri command 与 listen event

IPC mock 模块 SHALL 注入**全部**已注册的 Tauri command 与前端实际订阅的 listen event；listen event 覆盖范围 SHALL 至少含 `[[push-events]]` 定义的所有 push event webview event name，新增 push event variant SHALL 同步注入。覆盖率 SHALL 由自动化测试断言：mock 已知 command 列表 SHALL 与 Tauri 注册列表逐项对齐，缺一则用例 fail；listen event 名单 SHALL 与 `[[push-events]]` 定义的 event name 清单逐项对齐。未覆盖的 command 被前端 invoke 时 SHALL 返回明确的 `[mockIPC] command "<name>" not implemented` 错误而非静默 undefined。

仅注册为 Tauri command 的方法才在 mock 覆盖范围内；仅供 HTTP server 调用的内部方法 SHALL NOT 被 mock。

#### Scenario: 注入完整性回归

- **WHEN** 单元测试层跑 mock 完整性测试
- **THEN** 用例 SHALL 把 mock 已知 command 列表与 Tauri 注册列表逐项对齐断言
- **AND** 用例 SHALL 把 mock listen event 名单与 `[[push-events]]` 定义的 push event webview event name 清单逐项对齐断言
- **AND** 用例 SHALL 对每个已知 command 实际 invoke 一次，断言其返回非 undefined 的值或抛出明确错误（任一 command 静默 resolve undefined 都 SHALL 让测试 fail）
- **AND** 任一缺失（mock 漏注入 / Tauri 漏注册 / listen event 漏覆盖）SHALL 导致测试 fail

#### Scenario: 未实现 command 的明确报错

- **WHEN** 前端调用 mock 未实现的命令（如新加的后端 IPC 还未同步 mock）
- **THEN** 控制台 SHALL 输出 `[mockIPC] command "<name>" not implemented`，包含 command 名
- **AND** 调用方 invoke 的 Promise MUST reject 而非 resolve undefined

### Requirement: mockIPC 仅在 dev/test 环境启用

应用入口 SHALL 在挂载 App 之前同步检查注入条件：仅当开发模式 **且**（URL 含 `?mock=1` **或** 浏览器 window 对象不含 Tauri runtime 标记）时执行 mock 注入。生产 bundle 中整个 mock 模块 MUST 被构建工具 tree-shake 剔除。

#### Scenario: 真 Tauri 窗口完全旁路 mock

- **WHEN** 用户运行桌面应用（dev 或 release）
- **THEN** Tauri runtime 标记已注入，存在
- **AND** mock SHALL 不被激活，所有 invoke 调用走真 Tauri IPC
- **AND** Network/console 中 MUST 没有 `[mockIPC]` 字样

#### Scenario: 浏览器 dev server 自动启用 mock

- **WHEN** 维护者在浏览器打开 dev server 地址（不带 query string）
- **THEN** mock SHALL 自动激活，UI 显示 fixture 数据
- **AND** 依赖 IPC 的组件 MUST 能正常渲染数据而非「加载中」死状态

#### Scenario: E2E 测试用 fixture 显式指定

- **WHEN** E2E 用例 navigate 到 dev server 带 `?mock=1&fixture=<name>` 参数
- **THEN** mock SHALL 加载指定 fixture
- **AND** 用例可对该 fixture 已知数据做精确断言（项目数 / session 数 / 标题等）

#### Scenario: Production bundle 不含 mock 代码

- **WHEN** 跑生产构建
- **THEN** 产出的 JS 文件 MUST 不包含 mock 相关字符串（`mockIPC` / `__fixtures__` / fixture 中的虚构项目名）
- **AND** 此约束 SHALL 由专门的 bundle 纯度测试断言

### Requirement: Playwright 必须覆盖最小 user story 集

E2E 测试套件 SHALL 至少包含以下 5 个 user story 用例，每个独立 spec 文件：

- `startup-and-dashboard`：启动 → 看到 Sidebar + Dashboard 项目卡片
- `select-project-and-session`：点项目展开 sessions → 点 session 打开 SessionDetail tab
- `command-palette`：快捷键调出 CommandPalette → 输入文字 → 导航 → 选中
- `theme-switch`：切换 light/dark/system 主题 → 验证 theme attribute + 背景色变化
- `settings-and-notifications`：打开 Settings tab → 看到 Trigger CRUD/通知/外观三分区；打开 Notifications tab → 看到 unread badge

每个用例 MUST 在 30 秒内完成；总跑时 MUST 不超过 3 分钟。

#### Scenario: 主路径覆盖完整

- **WHEN** CI 执行 E2E 测试套件
- **THEN** 上述 5 个 user story 用例 SHALL 全部存在并通过
- **AND** 每个用例至少包含 1 个断言

#### Scenario: 命令面板快捷键跨平台

- **WHEN** E2E 测试在 macOS / Linux 任一平台跑命令面板用例
- **THEN** 平台对应的 Cmd/Ctrl+K 快捷键 SHALL 触发 CommandPalette 弹出
- **AND** 用例 MUST 不写平台分支判断

#### Scenario: 主题切换验证背景色生效

- **WHEN** 主题切换用例切换到 dark 主题
- **THEN** 用例 SHALL 断言 theme attribute 设为 dark 且 body 背景色等于深色 token 的 RGB 值
- **AND** 仅断言 attribute 是不充分的，MUST 同时验证 CSS 已生效

### Requirement: Playwright baseline screenshot 不进 git

E2E 测试配置 SHALL 不要求截图快照文件提交到 git。CI MUST 用 snapshot 更新模式跑，失败时上传测试报告与截图作为 CI artifact 供人审。本地开发 MUST 用 snapshot 更新模式重新生成。

#### Scenario: gitignore 覆盖测试产物

- **WHEN** 执行 E2E 测试
- **THEN** 生成的截图目录、测试报告目录、测试结果目录 SHALL 被 gitignore 忽略
- **AND** `git status` MUST 不显示这些路径为 untracked

#### Scenario: CI 失败上传 artifact

- **WHEN** CI 中 E2E 测试 job 失败
- **THEN** workflow SHALL 上传测试报告与截图作为 CI artifact
- **AND** PR reviewer MUST 能从 CI UI 下载查看视觉 diff

### Requirement: Vitest 单测覆盖纯逻辑层

单元测试层 SHALL 至少包含以下纯逻辑/store 单测：

- 主题应用逻辑：三种模式（light/dark/system）+ system 跟随系统偏好
- Tab store 状态机：tab 增删改、settings/notifications 单例 tab 语义、activeTab 切换、per-tab UI 状态隔离
- Sidebar store 状态机：pin/hide 状态机、宽度持久化、per-project prefs
- FileChange store 防抖：`dedupeRefresh` 合并并发调用的行为契约

不 SHALL 写组件 mount 测试（dumb 组件由 E2E 集成层覆盖）。

#### Scenario: store 状态机覆盖

- **WHEN** 单元测试层跑 Tab store 测试
- **THEN** 用例 SHALL 验证：开 settings tab 两次只产生 1 个 tab（单例）；activeTabId 切换时 per-tab UI state 不丢失；关闭非 active tab 时 activeTabId 不变
- **AND** 每个行为 MUST 独立用例

#### Scenario: theme 三种模式 attribute 设置

- **WHEN** 单元测试层调用主题应用函数传入 light / dark / system
- **THEN** theme attribute SHALL 直接设为传入值（不在 JS 层额外 query）
- **AND** system 模式由 CSS media query 接管，符合「浅色默认 + 深色覆写 + media query 跟随系统」约定

### Requirement: Rust IPC contract test 守护字段形状

IPC 契约测试模块 SHALL 为每个与 Tauri command 对应的公开接口方法提供至少一个 contract test，断言：（a）返回 JSON 顶层字段名是 camelCase；（b）omitted flag 字段命名遵循 `<原字段>Omitted` 规范；（c）internally-tagged enum 的 tag 值与 spec 一致；（d）Optional 字段在 None 时不出现在 JSON 中。

#### Scenario: list_projects 字段名契约

- **WHEN** contract test 调用 list_projects 接口并序列化为 JSON
- **THEN** 顶层 array 元素 SHALL 含字段 `id` / `path` / `displayName` / `sessionCount`
- **AND** MUST 不含 snake_case 形式 `display_name` / `session_count`

#### Scenario: get_session_detail 的 omitted flag 契约

- **WHEN** contract test 调用 get_session_detail 接口并断言返回 JSON
- **THEN** 含 omit 行为的字段 SHALL 用 `<原字段>Omitted: true` 表达——实际字段名：`dataOmitted`（image source 内）、`contentOmitted`（assistant response 内）、`outputOmitted`（tool execution 内）、`messagesOmitted`（subagent process 内）
- **AND** MUST 不出现 `omitImage` / `image_omitted` / `responseContentOmitted` / `toolOutputOmitted` 等其他命名变体

#### Scenario: ContextInjection internally-tagged enum

- **WHEN** contract test 序列化 ContextInjection claude-md variant
- **THEN** 输出 JSON SHALL 形如 `{ "category": "claude-md", "id": "...", ... }`
- **AND** MUST 不出现 externally-tagged 形式

#### Scenario: 新加 command 必须新加 contract test

- **WHEN** 维护者新增 Tauri command
- **THEN** PR CI SHALL 在没有对应 contract test 时失败（通过 contract test 的 command 列表断言或 review checklist）
- **AND** Tauri command 注册列表与 contract test 的 command 名列表 MUST 同步更新

### Requirement: CI 集成与本地一键跑

CI workflow SHALL 新增独立 `frontend-test` job，并行于现有 `rust-test` job。本地 SHALL 提供 `just test-ui` / `just test-e2e` 一键跑前端测试栈。

#### Scenario: CI job 独立运行

- **WHEN** PR 触发 CI
- **THEN** `frontend-test` job SHALL 与 `rust-test` job 并行启动
- **AND** 任一 job 失败 MUST 阻止 PR merge（branch protection 配置）

#### Scenario: 本地一键测试

- **WHEN** 维护者在仓库根目录跑 `just test-ui`
- **THEN** SHALL 顺序执行单元测试 + 类型检查
- **AND** 跑 `just test-e2e` SHALL 执行 E2E 集成测试套件

#### Scenario: CI 时间预算

- **WHEN** 前端测试 job 完成
- **THEN** 总耗时 SHALL 不超过 5 分钟（缓存命中场景）
- **AND** 缓存未命中场景下 SHALL 不超过 8 分钟


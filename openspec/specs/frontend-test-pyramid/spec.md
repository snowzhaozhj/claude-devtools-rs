# frontend-test-pyramid Specification

## Purpose
TBD - created by archiving change frontend-test-infrastructure. Update Purpose after archive.
## Requirements
### Requirement: 测试金字塔分四层且职责互斥

系统 SHALL 通过四层测试基础设施守护前端质量，每层职责互斥不重叠：（1）`mockIPC + Vite dev server` 提供 dev/test 环境的假后端；（2）`Playwright + Chromium` 跑 user story 级浏览器集成测试；（3）`Vitest + jsdom` 跑纯函数和 store 单元测试；（4）`crates/cdt-api/tests/ipc_contract.rs` 守护 IPC 字段形状契约。任何一层都 MUST 不被其他层替代。

#### Scenario: 改 UI 组件触发 Playwright 而非 vitest 组件测

- **WHEN** 维护者修改 `ui/src/components/Sidebar.svelte` 的渲染或交互
- **THEN** 回归覆盖 SHALL 由 `ui/tests/e2e/*.spec.ts` 中的 user story 用例提供，**不**由 vitest 组件单测提供
- **AND** Vitest 用例 MUST 不包含针对 dumb 渲染组件（BaseItem / StatusDot / OutputBlock 等）的 mount + assertion 测试

#### Scenario: 改 IPC command 字段触发 Rust contract test 而非 Playwright

- **WHEN** 维护者在 `crates/cdt-api` 修改 `LocalDataApi` 某方法返回 struct 的字段
- **THEN** `crates/cdt-api/tests/ipc_contract.rs` SHALL 至少有一个断言因字段名/形状变化而失败
- **AND** 该层失败 MUST 优先于 mockIPC fixture 同步——fixture 漂移是次生问题，契约测试是首要守护

#### Scenario: 加纯算法函数触发 vitest 而非 Playwright

- **WHEN** 维护者在 `ui/src/lib/` 新增一个纯函数（无 DOM、无 IPC 依赖，如 `formatDuration` / `parseUrl`）
- **THEN** 该函数 SHALL 由 `ui/src/lib/<name>.test.ts` 单元测试覆盖
- **AND** Playwright 用例 MUST 不为验证此类纯函数而存在

### Requirement: mockIPC 必须覆盖所有 Tauri command 与 listen event

`ui/src/lib/tauriMock.ts` SHALL 通过 `@tauri-apps/api/mocks` 的 `mockIPC()` 注入全部 22 个 Tauri IPC command（list_projects、list_sessions、get_session_detail、get_subagent_trace、get_image_asset、get_tool_output、search_sessions、get_config、update_config、get_notifications、mark_notification_read、delete_notification、mark_all_notifications_read、clear_notifications、add_trigger、remove_trigger、read_agent_configs、pin_session、unpin_session、hide_session、unhide_session、get_project_session_prefs）和 4 个 listen event（notification-update、notification-added、file-change、session-metadata-update）。未覆盖的 command 被前端 invoke 时 SHALL 返回明确的 `[mockIPC] command "<name>" not implemented` 错误而非静默 undefined。注意 `list_sessions_sync` 是 `LocalDataApi` 的公开方法但**不**注册为 Tauri command（仅供 HTTP server 调），因此不在 mockIPC 覆盖范围。

#### Scenario: 注入完整性回归

- **WHEN** vitest 跑 `ui/src/lib/tauriMock.test.ts`
- **THEN** 用例 SHALL 遍历所有已知 command 名（与 `src-tauri/src/lib.rs` 的 `invoke_handler!` 列表对照）
- **AND** 每个 command 调用都 MUST 返回非 undefined 的值或抛出明确错误

#### Scenario: 未实现 command 的明确报错

- **WHEN** 前端调用 mockIPC 未实现的命令（如新加的后端 IPC 还未同步 mock）
- **THEN** 控制台 SHALL 输出 `[mockIPC] command "<name>" not implemented`，包含 command 名
- **AND** 调用方 invoke 的 Promise MUST reject 而非 resolve undefined

### Requirement: mockIPC 仅在 dev/test 环境启用

`ui/src/main.ts` SHALL 在挂载 App 之前同步检查注入条件：仅当 `import.meta.env.DEV === true` **且**（URL 含 `?mock=1` **或** 浏览器 window 对象不含 `__TAURI_INTERNALS__`）时执行 `mockIPC()` 注入。生产 bundle 中整个 mockIPC 模块 MUST 被 Vite tree-shake 剔除。

#### Scenario: 真 Tauri 窗口完全旁路 mock

- **WHEN** 用户运行 `cargo tauri dev` 或 release 桌面应用
- **THEN** `window.__TAURI_INTERNALS__` 由 Tauri runtime 注入，存在
- **AND** mockIPC SHALL 不被激活，所有 invoke 调用走真 Tauri IPC
- **AND** Network/console 中 MUST 没有 `[mockIPC]` 字样

#### Scenario: 浏览器 vite dev 自动启用 mock

- **WHEN** 维护者在浏览器打开 `http://localhost:5173/`（不带 query string）
- **THEN** mockIPC SHALL 自动激活，UI 显示 fixture 数据
- **AND** Sidebar / Dashboard 等依赖 IPC 的组件 MUST 能正常渲染数据而非「加载中」死状态

#### Scenario: Playwright 用 fixture 显式指定

- **WHEN** Playwright 用例 navigate 到 `http://localhost:5173/?mock=1&fixture=multi-project-rich`
- **THEN** mockIPC SHALL 加载名为 `multi-project-rich` 的 fixture
- **AND** 用例可对该 fixture 已知数据做精确断言（项目数 / session 数 / 标题等）

#### Scenario: Production bundle 不含 mockIPC 代码

- **WHEN** 跑 `npm run build --prefix ui`
- **THEN** 产出的 `dist/assets/*.js` MUST 不包含字符串 `mockIPC` / `__fixtures__` / fixture 文件中的虚构项目名
- **AND** 此约束 SHALL 由专门 vitest 用例 `tauriMock.bundle.test.ts` grep 产物文件断言

### Requirement: Playwright 必须覆盖最小 user story 集

`ui/tests/e2e/` SHALL 至少包含以下 5 个 user story 用例，每个独立 spec 文件：

- `startup-and-dashboard.spec.ts`：启动 → 看到 Sidebar + Dashboard 项目卡片
- `select-project-and-session.spec.ts`：点项目展开 sessions → 点 session 打开 SessionDetail tab
- `command-palette.spec.ts`：`ControlOrMeta+K` 调出 CommandPalette → 输入文字 → ↑↓ 导航 → ↵ 选中
- `theme-switch.spec.ts`：切换 light/dark/system 主题 → 验证 `data-theme` attribute + 背景色变化
- `settings-and-notifications.spec.ts`：打开 Settings tab → 看到 Trigger CRUD/通知/外观三分区；打开 Notifications tab → 看到 unread badge

每个用例 MUST 在 30 秒内完成；总跑时 MUST 不超过 3 分钟。

#### Scenario: 主路径覆盖完整

- **WHEN** CI 跑 `npx playwright test --project chromium`
- **THEN** 上述 5 个 spec 文件 SHALL 全部存在并通过
- **AND** 每个 spec 至少包含 1 个 `expect(...)` 断言

#### Scenario: Cmd+K 跨平台

- **WHEN** Playwright 在 macOS / Linux 任一平台跑 `command-palette.spec.ts`
- **THEN** `await page.keyboard.press('ControlOrMeta+K')` SHALL 在两个平台都触发 CommandPalette 弹出
- **AND** 用例 MUST 不写平台分支判断

#### Scenario: 主题切换实测背景色

- **WHEN** `theme-switch.spec.ts` 切换到 dark 主题
- **THEN** 用例 SHALL 断言 `document.documentElement.dataset.theme === 'dark'` 且 `getComputedStyle(document.body).backgroundColor` 等于深色 token 的 RGB 值（如 `rgb(30, 30, 28)`）
- **AND** 仅断言 `data-theme` attribute 是不充分的，MUST 同时验证 CSS 已生效

### Requirement: Playwright baseline screenshot 不进 git

Playwright 配置 SHALL 不要求 baseline screenshot 文件提交到 git。CI MUST 用 `--update-snapshots` 模式跑，失败时上传 `playwright-report/` 和 screenshots 作为 GitHub Actions artifact 供人审。本地开发 MUST 用 `npx playwright test --update-snapshots` 重新生成 baseline。

#### Scenario: gitignore 覆盖 Playwright 产物

- **WHEN** 跑 `npx playwright test`
- **THEN** 生成的 `ui/tests/e2e/__screenshots__/`、`ui/playwright-report/`、`ui/test-results/` SHALL 被 `ui/.gitignore` 忽略
- **AND** `git status` MUST 不显示这些路径为 untracked

#### Scenario: CI 失败上传 artifact

- **WHEN** GitHub Actions 中 Playwright job 失败
- **THEN** workflow SHALL 用 `actions/upload-artifact@v4` 上传 `ui/playwright-report/` 与 screenshots
- **AND** PR reviewer MUST 能从 GitHub UI 下载查看视觉 diff

### Requirement: Vitest 单测覆盖纯逻辑层

`ui/src/**/*.test.ts` SHALL 至少包含以下纯逻辑/store 单测：

- `lib/theme.test.ts`：`applyTheme(mode)` 三种模式（light/dark/system）+ system 跟随 `prefers-color-scheme` media query
- `lib/tabStore.test.ts`：tab 增删改、settings/notifications 单例 tab 语义、activeTab 切换、per-tab UI 状态隔离
- `lib/sidebarStore.test.ts`：pin/hide 状态机、宽度持久化、per-project prefs
- `lib/fileChangeStore.test.ts`：`dedupeRefresh` 合并并发调用的行为契约

不 SHALL 写组件 mount 测试（dumb 组件由 Playwright 集成层覆盖）。

#### Scenario: store 状态机覆盖

- **WHEN** vitest 跑 `lib/tabStore.test.ts`
- **THEN** 用例 SHALL 验证：开 settings tab 两次只产生 1 个 tab（单例）；activeTabId 切换时 per-tab UI state 不丢失；关闭非 active tab 时 activeTabId 不变
- **AND** 每个行为 MUST 独立用例

#### Scenario: theme 三种模式 attribute 设置

- **WHEN** `lib/theme.test.ts` 调用 `applyTheme('light' | 'dark' | 'system')`
- **THEN** `document.documentElement.dataset.theme` SHALL 直接设为传入值（不在 JS 层 query matchMedia）
- **AND** `'system'` 模式由 `app.css` 中 `@media (prefers-color-scheme: dark)` 接管，符合现有 CLAUDE.md 「`:root` 浅色 + `[data-theme="dark"]` 深色 + `@media prefers-color-scheme` 跟随系统」约定

### Requirement: Rust IPC contract test 守护字段形状

`crates/cdt-api/tests/ipc_contract.rs` SHALL 为 `LocalDataApi` 的每个公开方法（与 Tauri command 1:1 对应）提供至少一个 contract test，断言：（a）返回 JSON 顶层字段名是 camelCase；（b）`xxxOmitted` flag 字段命名遵循 `<原字段>Omitted` 规范；（c）`#[serde(tag = "...")]` 的 internally-tagged enum tag 值与 spec 一致；（d）`#[serde(skip_serializing_if = "Option::is_none")]` 字段在 None 时不出现。

#### Scenario: list_projects 字段名契约

- **WHEN** contract test 调用 `api.list_projects().await` 并 `serde_json::to_value(&result)`
- **THEN** 顶层 array 元素 SHALL 含字段 `id` / `path` / `displayName` / `sessionCount`
- **AND** MUST 不含 snake_case 形式 `display_name` / `session_count`

#### Scenario: get_session_detail 的 omitted flag 契约

- **WHEN** contract test 调用 `api.get_session_detail(...)` 并断言返回 JSON
- **THEN** 含 omit 行为的字段 SHALL 用 `<原字段>Omitted: true` 表达——实际字段名（按 IPC 实现）：`dataOmitted`（image source 内）、`contentOmitted`（assistant response 内）、`outputOmitted`（tool execution 内）、`messagesOmitted`（subagent process 内）
- **AND** MUST 不出现 `omitImage` / `image_omitted` / `responseContentOmitted` / `toolOutputOmitted` 等其他命名变体（`<原字段>Omitted` 命名规范 SHALL 严格遵守）

#### Scenario: ContextInjection internally-tagged enum

- **WHEN** contract test 序列化 `ContextInjection::ClaudeMd { ... }`
- **THEN** 输出 JSON SHALL 形如 `{ "category": "claude-md", "id": "...", ... }`
- **AND** MUST 不出现 `{ "ClaudeMd": { ... } }` 这种 externally-tagged 形式

#### Scenario: 新加 command 必须新加 contract test

- **WHEN** 维护者在 `LocalDataApi` 新增公开方法 `xxx_yyy()`
- **THEN** PR CI SHALL 在没有对应 contract test 时失败（通过 contract test 文件的 command 列表断言或 manual review checklist）
- **AND** Tauri command 注册到 `invoke_handler!` 与 contract test 文件的 command 名列表 MUST 同步更新

### Requirement: CI 集成与本地一键跑

`.github/workflows/` SHALL 新增独立 frontend-test job，并行于现有 Rust workspace 测试。`justfile` SHALL 提供 `test-ui` / `test-e2e` recipe，本地一键跑前端测试栈。

#### Scenario: CI job 独立运行

- **WHEN** PR 触发 GitHub Actions
- **THEN** `frontend-test` job SHALL 与 `rust-test` job 并行启动
- **AND** 任一 job 失败 MUST 阻止 PR merge（branch protection 配置）

#### Scenario: 本地 just recipe

- **WHEN** 维护者在仓库根目录跑 `just test-ui`
- **THEN** SHALL 顺序执行 `npm run test:unit --prefix ui`（vitest）+ `npm run check --prefix ui`（svelte-check）
- **AND** 跑 `just test-e2e` SHALL 执行 `npm run test:e2e --prefix ui`（playwright）

#### Scenario: CI 时间预算

- **WHEN** frontend-test job 完成
- **THEN** 总耗时 SHALL 不超过 5 分钟（含 npm ci + chromium 安装的缓存命中场景）
- **AND** 缓存未命中场景下 SHALL 不超过 8 分钟


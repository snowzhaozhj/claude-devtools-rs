# frontend-test-pyramid delta — frontend-pnpm-migration

## MODIFIED Requirements

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

- **WHEN** 跑 `pnpm --dir ui build`
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

- **WHEN** CI 跑 `pnpm exec playwright test --project chromium`
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

Playwright 配置 SHALL 不要求 baseline screenshot 文件提交到 git。CI MUST 用 `--update-snapshots` 模式跑，失败时上传 `playwright-report/` 和 screenshots 作为 GitHub Actions artifact 供人审。本地开发 MUST 用 `pnpm exec playwright test --update-snapshots` 重新生成 baseline。

#### Scenario: gitignore 覆盖 Playwright 产物

- **WHEN** 跑 `pnpm exec playwright test`
- **THEN** 生成的 `ui/tests/e2e/__screenshots__/`、`ui/playwright-report/`、`ui/test-results/` SHALL 被 `ui/.gitignore` 忽略
- **AND** `git status` MUST 不显示这些路径为 untracked

#### Scenario: CI 失败上传 artifact

- **WHEN** GitHub Actions 中 Playwright job 失败
- **THEN** workflow SHALL 用 `actions/upload-artifact@v4` 上传 `ui/playwright-report/` 与 screenshots
- **AND** PR reviewer MUST 能从 GitHub UI 下载查看视觉 diff

### Requirement: CI 集成与本地一键跑

`.github/workflows/` SHALL 新增独立 frontend-test job，并行于现有 Rust workspace 测试。`justfile` SHALL 提供 `test-ui` / `test-e2e` recipe，本地一键跑前端测试栈。

#### Scenario: CI job 独立运行

- **WHEN** PR 触发 GitHub Actions
- **THEN** `frontend-test` job SHALL 与 `rust-test` job 并行启动
- **AND** 任一 job 失败 MUST 阻止 PR merge（branch protection 配置）

#### Scenario: 本地 just recipe

- **WHEN** 维护者在仓库根目录跑 `just test-ui`
- **THEN** SHALL 顺序执行 `pnpm --dir ui test:unit`（vitest）+ `pnpm --dir ui check`（svelte-check）
- **AND** 跑 `just test-e2e` SHALL 执行 `pnpm --dir ui test:e2e`（playwright）

#### Scenario: CI 时间预算

- **WHEN** frontend-test job 完成
- **THEN** 总耗时 SHALL 不超过 5 分钟（含 `pnpm install --frozen-lockfile` + chromium 安装的缓存命中场景）
- **AND** 缓存未命中场景下 SHALL 不超过 8 分钟

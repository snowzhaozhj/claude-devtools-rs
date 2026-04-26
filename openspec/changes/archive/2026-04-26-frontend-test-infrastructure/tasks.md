## 1. Vitest 与 Rust contract test 基础设施

- [x] 1.1 `ui/package.json` 加 devDependencies：`vitest@^4`、`@vitest/ui@^4`、`jsdom@^29`、`@testing-library/svelte@^5`、`@testing-library/jest-dom@^6`（实装版本，原 design D7 写的 ^2 与 vite ^8 不兼容，反转记录见 design D7b）
- [x] 1.2 新建 `ui/vitest.config.ts`：与 `vite.config.ts` 共享 plugin 配置（`mergeConfig`），`environment: 'jsdom'`，`setupFiles: ['./src/test-setup.ts']`
- [x] 1.3 新建 `ui/src/test-setup.ts`：import `@testing-library/jest-dom/vitest`，注入 matchMedia mock helper
- [x] 1.4 `ui/package.json` 加 scripts：`test:unit` (`vitest run`)、`test:unit:watch` (`vitest`)
- [x] 1.5 新建 `crates/cdt-api/tests/ipc_contract.rs` 骨架：tokio test runtime、setup_api 共享 fixture helper、列出全部 22 个 Tauri command 名常量数组（与 `src-tauri/src/lib.rs::invoke_handler!` 列表对照）
- [x] 1.6 `cargo test -p cdt-api --test ipc_contract`（3 个骨架用例通过）+ `cargo clippy -p cdt-api --tests -- -D warnings`

## 2. Rust IPC contract 用例（按 command 一组）

- [x] 2.1 `list_projects` / `list_sessions` / `list_sessions_sync` 字段名 camelCase 断言（`displayName`/`sessionCount`/`projectId`/`sessionId`/`messageCount`/`isOngoing`）
- [x] 2.2 `get_session_detail` 完整 JSON 形状：chunk 类型 tag (`user`/`ai`/`system`/`compact`，tag key 为 `kind`)、AIChunk 内嵌字段（`teammateMessages`/`slashCommands`/`responses`/`toolExecutions`/`subagents`）+ omitted flag (`dataOmitted`/`contentOmitted`/`outputOmitted`/`messagesOmitted`)
- [x] 2.3 `get_subagent_trace` / `get_image_asset` / `get_tool_output` 懒加载 IPC 字段形状
- [x] 2.4 `get_config` / `update_config` 形状（含 `notifications.triggers` 数组、顶层 `httpServer` 等 camelCase sections）
- [x] 2.5 `get_notifications` / `mark_notification_read` / `delete_notification` / `mark_all_notifications_read` / `clear_notifications` 形状
- [x] 2.6 `add_trigger` / `remove_trigger` / `read_agent_configs` / `pin_session` / `unpin_session` / `hide_session` / `unhide_session` / `get_project_session_prefs` 形状
- [x] 2.7 `ContextInjection` internally-tagged enum 序列化形状（`category: "claude-md"` 等 6 个 category）
- [x] 2.8 一致性检查：`crates/cdt-api/tests/ipc_contract.rs` 顶层 `EXPECTED_TAURI_COMMANDS: &[&str]` 长度断言（22）+ 去重断言
- [x] 2.9 `cargo test -p cdt-api --test ipc_contract` 全绿（43 用例）+ `cargo clippy -p cdt-api --tests -- -D warnings`

## 3. mockIPC 注入与 fixture 体系

- [x] 3.1 新建 `ui/src/lib/__fixtures__/types.ts`：复用 `ui/src/lib/api.ts` 的 `ProjectInfo`/`SessionSummary`/`SessionDetail`/`AppConfig`/`GetNotificationsResult`/`AgentConfig` 等类型，导出 `Fixture` interface
- [x] 3.2 新建 `ui/src/lib/__fixtures__/empty.ts`：0 项目场景
- [x] 3.3 新建 `ui/src/lib/__fixtures__/single-project.ts`：1 项目 + 1 session（含 SessionDetail：1 UserChunk + 1 AIChunk 含 1 ToolExecution）
- [x] 3.4 新建 `ui/src/lib/__fixtures__/multi-project-rich.ts`：5 项目（rust-port/claude-devtools/docs/experiment/archive）× 多会话（含 ongoing session、pin/hide 状态、teammateMessages、slashCommands、interruption SemanticStep、SystemChunk、CompactChunk）
- [x] 3.5 新建 `ui/src/lib/__fixtures__/index.ts`：`selectFixture(name)` + 3 个 fixture 注册
- [x] 3.6 新建 `ui/src/lib/tauriMock.ts`：导出 `setupMockIPC(fixtureName)`，覆盖全部 22 个 Tauri command + `mockWindows('main')` + `shouldMockEvents: true`
- [x] 3.7 未实现 command 的兜底：default 分支 console.warn `[mockIPC] command "<name>" not implemented` 并 reject `UnknownCommandError`；`plugin:*` 内部 command 由 `shouldMockEvents` 自动 no-op
- [x] 3.8 `ui/src/main.ts` 加 dev-only 注入逻辑：`if (import.meta.env.DEV) { ...; const { setupMockIPC } = await import('./lib/tauriMock'); setupMockIPC(...) }`，整个 if 块在 production 由 esbuild DCE 剔除（含 dynamic import chunk）
- [x] 3.9 新建 `ui/src/lib/tauriMock.test.ts`：vitest 用例 25 个（`test.each(KNOWN_TAURI_COMMANDS)` 22 + 未知命令 + listen 不抛 + 4 个真实 event 挂载）全过
- [x] 3.10 新建 `ui/src/lib/tauriMock.bundle.test.ts`：`RUN_BUNDLE_TESTS=1` 触发，跑 `npm run build`（NODE_ENV=production）后 grep `dist/assets/*.js`，断言不含 `mockIPC` / `__fixtures__` / 虚构项目名 — 实测 production bundle **完全不输出** tauriMock chunk
- [x] 3.11 `npm run check --prefix ui`（0 errors，5 个预存在 warning）+ `npm run test:unit --prefix ui`（26 用例 + 1 bundle test 都通过）

## 4. Vitest 纯逻辑/store 单测

- [x] 4.1 `ui/src/lib/theme.test.ts`：`applyTheme('light'|'dark'|'system')` 三种 attribute 设置（'system' 由 CSS @media 处理，不需 matchMedia mock，已在 design D7c 记录修订）
- [x] 4.2 `ui/src/lib/tabStore.test.ts`：openSettingsTab/openNotificationsTab 单例语义；setActiveTab 切换；getTabUIState/saveTabUIState per-tab 隔离
- [x] 4.3 `ui/src/lib/sidebarStore.test.ts`：setSidebarWidth 边界 clamp（200~500）；pin/hide 状态机已由 mockIPC 集成测覆盖，本文件不重复
- [x] 4.4 `ui/src/lib/fileChangeStore.test.ts`：dedupeRefresh 同 key 并发合并 / 不同 key 各跑 / resolve 后再触发 / reject 不污染 inFlight 五个用例
- [x] 4.5 `npm run test:unit --prefix ui` 全绿（5 个 test file 43 用例 + 1 跳过 bundle test）

## 5. Playwright 安装与配置

- [x] 5.1 `ui/package.json` 加 devDependencies：`@playwright/test@^1.59`
- [x] 5.2 `ui/package.json` 加 scripts：`test:e2e` / `test:e2e:ui` / `test:e2e:update`
- [x] 5.3 新建 `ui/playwright.config.ts`：testDir / chromium-only project / webServer 复用 vite dev / baseURL / trace+screenshot 失败时保留 / `toHaveScreenshot.maxDiffPixelRatio: 0.02`
- [x] 5.4 `ui/.gitignore` 加 `tests/e2e/__screenshots__/` / `playwright-report/` / `test-results/` / `playwright/.cache/`
- [x] 5.5 `npx playwright install chromium` 完成（本地 Chromium 装好）+ `npx playwright test --list` 列出 0 用例不报错

## 6. Playwright user story 用例

- [x] 6.1 新建 `ui/tests/e2e/startup-and-dashboard.spec.ts`：`?mock=1&fixture=multi-project-rich` → 断言 sidebar header「选择项目」/项目名 + Dashboard「最近项目」+ 项目数 + screenshot sanity
- [x] 6.2 新建 `ui/tests/e2e/select-project-and-session.spec.ts`：点 Dashboard 卡片 → sidebar 显示 sessions（test 1）；通过 `__cdtTest.openTab` 直接打开 SessionDetail tab → TabBar 出现（test 2，避免 sidebar virtualization 导致的 click flake）
- [x] 6.3 新建 `ui/tests/e2e/command-palette.spec.ts`：`document.dispatchEvent` 模拟 Cmd/Ctrl+K → 断言 CommandPalette 弹出 + 键位提示 + 输入搜到 + Esc 关闭
- [x] 6.4 新建 `ui/tests/e2e/theme-switch.spec.ts`：`setAttribute('data-theme', mode)` → 断言 `getComputedStyle(body).backgroundColor` light vs dark 差异（dark RGB 三通道和 < 300）；poll 等 mockIPC config load 完成后再循环测三种 mode attribute 设置
- [x] 6.5 新建 `ui/tests/e2e/settings-and-notifications.spec.ts`：通过 `__cdtTest.openSettingsTab` / `openNotificationsTab` 直接打开（避开 TabBar 仅在 pane 有 tab 时显示的约束）→ Settings 看到「常规」section + 主题 + 切「通知」sub-tab 看 h3「通知」+「启用通知」/ 触发器描述；Notifications 看「1 条未读」
- [x] 6.6 startup-and-dashboard / select-project-and-session 各 1 个 `expect(page).toHaveScreenshot()` sanity（其他 spec 不强加，避免 CI 噪声）
- [x] 6.7 `npx playwright test --workers=1`：11 个用例全通过，总耗时 12s（远低于 3 分钟）

## 7. Justfile 与 CI 集成

- [x] 7.1 `justfile` 加 recipe：`test-ui-unit`（vitest）/ `test-ui`（vitest + svelte-check）/ `test-e2e`（playwright）
- [x] 7.2 `justfile` 的 `preflight` recipe 加 `test-ui-unit` 一步（不加 e2e，e2e 较慢只在 CI / 手动跑）
- [x] 7.3 新建 `.github/workflows/frontend-test.yml`：on PR + push to main；jobs `unit`（npm ci + vitest 含 RUN_BUNDLE_TESTS=1 + svelte-check）+ `e2e`（npm ci + playwright install + playwright test workers=2 + 失败上传 playwright-report/ + test-results/ artifact）
- [x] 7.4 GitHub Actions cache：`actions/setup-node@v4` with `cache: 'npm' cache-dependency-path: ui/package-lock.json` + `actions/cache@v4` Playwright browser cache `~/.cache/ms-playwright`
- [ ] 7.5 推 PR 验证 frontend-test job 在 GitHub 跑通（待 PR 创建后实测；本地 `just test-ui` 已绿，CI 配置等推送后看实跑）

## 8. 文档与收尾

- [x] 8.1 `CLAUDE.md` 加「IPC 字段改动 checklist」：改 `LocalDataApi` 公开方法返回字段时 SHALL 同 PR 同步 (a) `cdt-api/tests/ipc_contract.rs` (b) `ui/src/lib/api.ts` interface (c) `ui/src/lib/__fixtures__/*.ts` (d) 新加 command 时同步三处 EXPECTED 列表
- [x] 8.2 `CLAUDE.md` 加「测试金字塔速查」表：4 层职责 + 跑命令 + 何时改 + 浏览器调试入口（`?mock=1&fixture=...`）
- [x] 8.3 `README.md` 的「常用 recipe」表补充 `just test-ui-unit` / `test-ui` / `test-e2e` + 「浏览器调试 UI」段说明 `?mock=1&fixture=...`
- [x] 8.4 `just preflight` 全绿（fmt + lint + cargo test + 前端 vitest + spec validate）
- [x] 8.5 `openspec validate frontend-test-infrastructure --strict` 全绿
- [ ] 8.6 PR 包含 archive commit：`/opsx:archive frontend-test-infrastructure`（archive commit 作为 PR 最后一个 commit；待用户确认推 PR 后做）

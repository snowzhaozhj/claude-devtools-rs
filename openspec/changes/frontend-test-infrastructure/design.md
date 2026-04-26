## Context

仓库当前状态：13 个数据 capability（cdt-parse / cdt-analyze / cdt-discover / cdt-watch / cdt-config / cdt-ssh / cdt-api）+ 6 个 UI 行为 spec（teammate-message-rendering、session-detail-lazy-render、subagent-messages-lazy-load、session-detail-image-asset-cache、session-detail-response-content-omit、session-ongoing-stale-check 等）已实现。Rust 侧测试覆盖完整（每 crate 单元测 + cdt-api/tests/ 集成测 + insta 快照），UI 侧零自动化测试，全靠 `just dev` 桌面窗口手动验证。

约束：
- **真 E2E 不在范围**：`tauri-driver` + `WebdriverIO` 在 macOS 上历来 flaky，1-2 个工作日设置 + 持续 CI 维护成本，本期不上
- **mockIPC 与真后端漂移是固有风险**：必须有兜底机制，不能依赖手动同步
- **不破坏 Tauri 桌面窗口**：mock 必须 dev-only + opt-in，真 `cargo tauri dev` 完全旁路
- **CI 时间预算**：现有 workflow 跑 ~6 分钟（Rust workspace + svelte-check），本期新增不超过 +3 分钟
- **macOS 主开发环境**：本地实测必须 macOS arm64 跑通，Linux CI 跑通，Windows 暂不强制

利益相关方：
- 主要用户=维护者本人（auto memory `project_ui_todo.md`：v0.1.1 已 release，剩余 UI 待办「等痛点再做」）
- 测试基础设施一旦立起来，后续 UI iteration 都受益

## Goals / Non-Goals

**Goals:**

- 改一个 UI 文件后 1 分钟内能在浏览器看到回归结果（不用起 Tauri 窗口）
- 加新 IPC command 时，必须同步加 contract test，否则 CI 红
- 5-10 个 user story 用例覆盖主路径：启动 → 选项目 → 看会话 → 搜索 → 切主题 → 查看通知/设置 → Sidebar 拖拽 → 右键菜单
- mockIPC fixture 形状漂移在 PR 阶段被捕获，不到 release 后才发现
- 测试金字塔写进 spec，未来换框架时按 spec delta 改

**Non-Goals:**

- 真 Tauri webview ↔ 真后端的 IPC 通讯测试（要 tauri-driver，本期跳过）
- macOS 平台 API（通知、托盘、setBadgeCount）自动化测试（同上）
- 浏览器多内核兼容（webkit / firefox 不测，chromium-only）
- 视觉 diff 的 AI 工具（chromatic 之类）
- 性能/负载测试（已有 `perf_get_session_detail` 兜底）
- 把 dumb 组件（BaseItem / StatusDot）做单独单测（Playwright 集成层已覆盖）

## Decisions

### D1: mockIPC 注入入口

**选择**：`ui/src/main.ts` 顶部，在 `mount(App, ...)` 之前同步注入。

**为什么**：组件挂载时 `$effect` 立即触发 `listProjects()` 等 invoke 调用（见 `Sidebar.svelte:118` / `DashboardView.svelte:44`）。如果把注入放到 `App.svelte` 的 setup 函数，挂载时 invoke 已被调用，必然报错。`main.ts` 是模块顶层，import 阶段已执行。

**触发条件**：`import.meta.env.DEV && (window.location.search.includes('mock=1') || !('__TAURI_INTERNALS__' in window))`
- 真 `cargo tauri dev` 窗口：有 `__TAURI_INTERNALS__`，不注入
- `vite dev` 浏览器访问：无 `__TAURI_INTERNALS__`，自动注入
- 强制 mock：URL 加 `?mock=1`（用于 Playwright 跑指定 fixture）

**Alternative 考虑**：放 `App.svelte` 的 onMount——挂载时机太晚，已被否；动态 lazy import——`main.ts` 同步阻塞 await dynamic import 反而更慢，已否。

### D2: Fixture 组织结构

**选择**：`ui/src/lib/__fixtures__/` 下按场景分目录，每个场景导出统一的 `Fixture` 接口。

```
ui/src/lib/__fixtures__/
├── types.ts                    # Fixture interface
├── empty.ts                    # 0 项目
├── single-project.ts           # 1 项目 + 1 session
├── multi-project-rich.ts       # 5 项目 × 多会话 + 含 ongoing
├── ongoing-session.ts          # 单会话 + isOngoing=true + 实时 metadata broadcast
└── index.ts                    # selectFixture(name: string): Fixture
```

URL `?mock=1&fixture=multi-project-rich` 选择具体 fixture。Playwright 用例显式指定。

**为什么放 src 下而不是 tests/**：dev 模式下浏览器调样式时也能 `?mock=1` 直接用，不用复制一份；Vite 自动 tree-shake，不会进 release bundle（`__fixtures__` 目录被 mockIPC 模块独占引用，整个 mockIPC 在 `import.meta.env.PROD` 时被 dead-code-elimination）。

**Alternative**：放 `ui/tests/fixtures/`——只能测试用，dev 调样式不能复用，已否。

### D3: Playwright vite server 策略

**选择**：`playwright.config.ts` 的 `webServer` 字段配 `npm run dev --prefix ui`，复用 vite dev server。

**为什么**：dev server HMR 在 Playwright 用例间不会触发（每个 test 之间没改文件），冷启动 ~1.5s vs `vite build && vite preview` 冷启动 ~5s。CI 启 dev 也 OK，HMR 不影响测试。

**Alternative 考虑**：`vite build` + `vite preview`——更接近生产，但慢 + 每次 build 浪费 CI 时间，已否；现成的 dev server（手动起好 playwright 不管）——CI 不方便，已否。

### D4: Rust IPC contract test 放置位置

**选择**：`crates/cdt-api/tests/ipc_contract.rs`，**不**放 `src-tauri/tests/`。

**为什么**：Tauri command 本质是 `LocalDataApi` 的薄包装（`State<LocalDataApi>` 注入 + `Result<Value, String>` 错误转换）。真正的 IPC 契约（字段名 camelCase / omitted flag 形状 / enum tag 值）由 `LocalDataApi` 返回的 struct 的 serde derive 决定。`cdt-api` 已有完整集成测脚手架（fixture 复用 `tests/fixtures/`），加一份契约测是自然延伸。

测试形态：
```rust
#[tokio::test]
async fn list_projects_returns_camelcase_fields() {
    let api = setup_api().await;
    let projects = api.list_projects().await.unwrap();
    let json = serde_json::to_value(&projects).unwrap();
    assert_eq!(json[0]["displayName"], json!(...));   // not "display_name"
    assert!(json[0].get("display_name").is_none());
    // ...
}
```

`src-tauri` 的 Tauri command 层不需独立 contract test——其 body 不超过 3 行（拿 State → 调 LocalDataApi → 转错误），出 bug 概率约等于 0；Tauri framework 自己保证 invoke handler 序列化路径。

**Alternative 考虑**：`src-tauri/tests/ipc_contract.rs`——需要 src-tauri 暴露 invoke handler 或绕 `tauri::test`，复杂；同 `lib.rs` 的 `#[cfg(test)] mod tests`——污染 lib.rs 且 src-tauri 独立 manifest 的测试 cache 经常爆（CLAUDE.md 已记录此坑），已否。

### D5: Playwright baseline screenshot 策略

**选择**：baseline screenshot **不 commit 进 git**；CI 跑时 `--update-snapshots` + 失败上传 artifact 供人审。本地开发时 `npx playwright test --update-snapshots` 重生成。

**为什么**：跨 OS（macOS arm64 / Linux）+ 跨 Chromium 版本会有亚像素 diff，commit baseline 会让 CI 在版本升级 / 切换 runner 时全部红。Playwright 的 `expect().toHaveScreenshot()` 默认对比 ref 文件，找不到 ref 时第一次自动生成——CI 上每次都生成，等于退化成「截图能截下来不报错」检查 + 视觉 artifact 给人审。这层主要靠**用 selector + role-based assertion** 守护（`expect(page.getByRole('button', { name: '选择项目' })).toBeVisible()`），screenshot 只做 sanity。

**Alternative 考虑**：commit baseline + 跨 OS 多套 baseline——维护成本太高；用 percy.io / chromatic——过度工程，本期跳过；不截图——失去视觉回归 + CI artifact 给人审的能力，已否。

### D6: mockIPC fixture 与真后端漂移防御

**选择**：双层防御：
- **强制层**：`crates/cdt-api/tests/ipc_contract.rs` 断言每个 API 方法返回的 JSON 形状（camelCase 字段名 / omitted flag 命名规范 / 必有字段列表）。后端字段一改，contract test 红
- **约定层**：`ui/src/lib/__fixtures__/types.ts` 用 TypeScript 类型导出 `ProjectInfo` / `SessionSummary` / `SessionDetail` 等接口（**复用** `ui/src/lib/api.ts` 已有的接口），fixture 数据必须实现这些 interface。后端字段改时前端类型先同步（这是常规 IPC 改动流程的一环），fixture TS 编译失败立即知道

**Nice-to-have（不在本期）**：写脚本 `cargo run --bin gen_fixtures` 真调 LocalDataApi 跑出 JSON，dump 成 fixture——能彻底消除手写漂移，但工程量大，等手写 fixture 维护痛了再做。

### D7: Vitest 配置 + Svelte 5 runes 支持

**选择**：`vitest` ^2.x + `@testing-library/svelte` ^5.x（runes 原生支持）+ `jsdom` 环境。配 `vitest.config.ts` 与 `vite.config.ts` 共享 plugin 配置（`mergeConfig`）。

**为什么**：`@testing-library/svelte` 5.x 是 Svelte 5 runes 兼容版本（4.x 不支持 runes），vitest 2.x 与之搭配最新。Svelte 5 的 `$state` / `$derived` / `$effect` 在 jsdom 里跑 OK，纯 store 测试不依赖 DOM 时连 jsdom 都不需要（`environment: 'node'`）。

**Alternative 考虑**：jest——vite 生态默认 vitest，配置共用更简单；happy-dom——比 jsdom 快，但 Svelte 测试社区主流是 jsdom，先稳定。

### D7c: theme 'system' 模式不在 JS 层跟随 matchMedia（apply 阶段实测修订）

**选择反转**：spec 原描述「`applyTheme('system')` 跟随 `prefers-color-scheme` media query」与代码实际不符——`ui/src/lib/theme.ts` 只设 `data-theme=<mode>`，'system' 模式由 `app.css` 的 `@media (prefers-color-scheme: dark)` 接管（CLAUDE.md 已记录此设计）。`theme.test.ts` 仅测 attribute 设置，不模拟 matchMedia，与实际语义对齐。

### D7b: Vitest 版本反转为 ^4.x（apply 阶段实测修订）

**选择反转**：实测 `vitest` ^2.x 与仓库现有 `vite` ^8.0.4 不兼容（vitest 2.x peer deps 限定 `vite ^5`，vitest 3.x 限定 `vite ^5 || ^6`）。`npm install vitest` 默认 resolve 到 ^4.1.5（peer `vite ^6 || ^7 || ^8`），与 `vite ^8` 兼容。`@vitest/ui` 跟随 vitest 主版本同步到 ^4.1.5；`jsdom` 升到 ^29；`@testing-library/svelte` ^5.3 不变。

**保留原 D7**：原决策的「vitest + @testing-library/svelte 5.x + jsdom」技术栈方向不变，仅版本号反转。

### D8: Playwright 跨平台快捷键

**选择**：用 `ControlOrMeta` 修饰符（Playwright 内置跨平台 alias），统一写 `await page.keyboard.press('ControlOrMeta+K')`。

**为什么**：CommandPalette 在 macOS 是 Cmd+K，Linux/Windows 是 Ctrl+K，Playwright 的 `ControlOrMeta` 自动适配。CI 跑 Linux runner 时不用改 test 代码。

### D9: CI workflow 集成方式

**选择**：在现有 `.github/workflows/` 加新 `frontend-test.yml` job（与 Rust workspace 测试并行），不改现有 workflow。

job 步骤：
1. checkout
2. setup-node (with cache)
3. `cd ui && npm ci`
4. `npx playwright install --with-deps chromium`（缓存）
5. `npm run test:unit` (vitest)
6. `npm run test:e2e` (playwright)
7. 失败时上传 `playwright-report/` + screenshots artifact

Rust IPC contract test 跟 `cargo test --workspace` 一起跑（已有 job，contract test 在 cdt-api crate 内自动包含）。

**为什么独立 job**：node + browser 缓存与 Rust 缓存独立，并行能省 2-3 分钟。

## Risks / Trade-offs

- **mockIPC 与真后端漂移**（D6 已部分缓解）→ Rust contract test 兜底字段命名 + TS 接口共享兜底字段存在性；剩余漂移（如字段语义变化但形状不变）只能靠 manual smoke 兜底。文档化：`CLAUDE.md` 加一段「IPC 字段改动 checklist」明确要同步改 fixture
- **Playwright 在 macOS 本地 vs Linux CI 字体渲染差异** → 用 `mask: [page.locator('.dynamic-content')]` + 容忍像素阈值 `maxDiffPixelRatio: 0.02` + 不 commit baseline (D5)
- **Vite HMR 干扰 Playwright** → 实测可控（Playwright 不改文件），如真有问题可用 `VITE_DISABLE_HMR=1` 环境变量关掉
- **CI 时间 +2-3 分钟** → 浏览器 binary 缓存（actions/cache）+ chromium-only 控制规模
- **Svelte 5 runes 在 vitest jsdom 环境的稳定性** → 实测 + 遇坑时可降级到只测 store/util 不测组件渲染
- **fixture 体量膨胀** → 强制约定每个 fixture 文件 < 200 行，超出拆子文件
- **本期暴露不出但未来可能踩**：mockIPC 测试通过 ≠ 真窗口能跑（Tauri command 错误处理路径、State 注入失败等）→ release-check 仍保留 `just dev` 手动 smoke

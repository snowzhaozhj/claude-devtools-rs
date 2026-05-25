## Context

`frontend-test-pyramid` spec 在 port 期写入了大量实现细节（文件路径、CLI 命令、Rust 类型签名），累计 48 处 spec-purity 命中。该 spec 较特殊：它描述测试基础设施本身，因此工具名（Playwright / Vitest / mockIPC）既是实现选择也是层级标识符。清理时需区分"层级身份标识"（保留）与"具体文件路径/命令/签名"（移除）。

当前 baseline `scripts/spec-purity-baseline.txt` 中 `spec/frontend-test-pyramid 48`。

## Goals / Non-Goals

**Goals:**

- 将 48 处 spec-purity 命中降至接近 0（目标 ≤ 5，接受少量工具名不可避免命中）
- 行为契约语义 100% 保持不变——外部可观察行为不改
- 将移除的实现细节作为"参考实现指引"记录在本 design.md

**Non-Goals:**

- 不改代码 / 测试文件 / CI 配置
- 不改 Requirement 顺序
- 不拆分 / 合并 Requirement
- 不改其它 capability 的 spec

## Decisions

### D1：工具名处理策略

**问题**：Playwright / Vitest / mockIPC / invoke_handler! 等词同时是 p6 命中源和 spec 的核心概念。

**决策**：用抽象层级名替代具体工具名——"E2E 集成测试层"、"单元测试层"、"IPC mock 层"、"IPC 契约测试层"。仅在 Purpose 一处以括号注明当前选型（`（当前实现：Playwright）`），其余 Requirement body 不再出现工具名。

**理由**：SPEC_GUIDE 明确把 vitest/playwright/svelte-check 归入"库与框架选型 → design"。即使本 spec 描述测试基础设施，行为契约仍应是工具无关的——理论上换成 Cypress 或 Jest 只要四层契约不变。

### D2：文件路径 / 测试文件名处理策略

**问题**：26 处 p2 命中含 `ui/src/...`、`crates/...`、`*.spec.ts`、`*.test.ts` 等。

**决策**：全部移除，改为抽象描述：
- 源码路径 → "Sidebar 组件"、"mock 模块"、"入口文件"等
- 具体测试文件名 `startup-and-dashboard.spec.ts` → 用 user story 名引述："启动与 Dashboard 展示"
- `ui/src/lib/<name>.test.ts` → "对应同名单元测试文件"
- `crates/cdt-api/tests/ipc_contract.rs` → "IPC 契约测试模块"

**理由**：文件路径是代码组织决策，重命名 / 移动不应破 spec。

### D3：Rust 类型签名 / 具体断言表达式

**问题**：`serde_json::to_value(&result)`、`#[serde(tag = "...")]`、`api.list_projects().await` 等。

**决策**：改为行为描述——"序列化为 JSON 后"、"internally-tagged enum 的 tag 值"、"调用列出项目接口"。

### D4：CLI 命令 / CI 工具调用

**问题**：`pnpm exec playwright test --project chromium`、`pnpm --dir ui test:unit` 等。

**决策**：抽象为"CI 执行 E2E 测试套件"、"本地一键前端测试"等。具体命令移至本 design.md 参考实现指引。

### D5：p4 命中（metric/baseline/实测）

**问题**：3 处——"实测背景色"（Scenario 标题）、"baseline screenshot"（Requirement 标题 + body）。

**决策**：
- "实测背景色" → "验证背景色生效"
- "baseline screenshot" → "截图快照"（"snapshot" 是测试术语非 metric）

## Risks / Trade-offs

- [可读性] 过度抽象可能让新人难理解四层分别指什么 → Mitigation：Purpose 段保留括号注释当前选型
- [残留命中] 工具名完全不出可能剩 2-5 处灰色命中（如 `mockIPC` 作为功能名而非库名）→ Mitigation：接受少量，目标 ≤ 5 而非 0

## 参考实现指引（从 spec 移出的实现细节）

以下为当前实现对应关系，供维护者参考：

### 四层对应文件

| 层级 | 当前实现 | 测试入口 |
|---|---|---|
| IPC 契约测试 | `crates/cdt-api/tests/ipc_contract.rs` | `cargo test -p cdt-api --test ipc_contract` |
| 单元测试 | `ui/src/**/*.test.ts` (Vitest + jsdom) | `pnpm --dir ui test:unit` |
| E2E 集成测试 | `ui/tests/e2e/*.spec.ts` (Playwright + Chromium) | `pnpm exec playwright test` |
| IPC mock 层 | `ui/src/lib/tauriMock.ts` (@tauri-apps/api/mocks) | dev server 自动注入 |

### E2E 最小 user story 集文件映射

| User story | 当前文件 |
|---|---|
| 启动与 Dashboard 展示 | `startup-and-dashboard.spec.ts` |
| 选择项目与 session | `select-project-and-session.spec.ts` |
| 命令面板 | `command-palette.spec.ts` |
| 主题切换 | `theme-switch.spec.ts` |
| 设置与通知 | `settings-and-notifications.spec.ts` |

### 单元测试最小集文件映射

| 测试目标 | 当前文件 |
|---|---|
| 主题应用逻辑 | `lib/theme.test.ts` |
| Tab store 状态机 | `lib/tabStore.test.ts` |
| Sidebar store 状态机 | `lib/sidebarStore.test.ts` |
| FileChange store 防抖 | `lib/fileChangeStore.test.ts` |

### mock 注入入口

- 入口文件：`ui/src/main.ts`
- mock 模块：`ui/src/lib/tauriMock.ts`
- 完整性断言：`ui/src/lib/tauriMock.test.ts`
- bundle 纯度：`ui/src/lib/tauriMock.bundle.test.ts`
- Tauri 注册宏：`src-tauri/src/lib.rs::invoke_handler!`
- mock 库：`@tauri-apps/api/mocks`

### IPC 契约测试断言模式

- 调用：`api.list_projects().await` + `serde_json::to_value(&result)`
- camelCase 检查：顶层字段名
- omitted flag：`<原字段>Omitted: true`
- internally-tagged enum：`#[serde(tag = "...")]` → `{ "category": "claude-md", ... }`
- skip_serializing_if：`#[serde(skip_serializing_if = "Option::is_none")]`

### CI 与本地命令

- CI E2E：`pnpm exec playwright test --project chromium`
- CI 单元测试：`pnpm --dir ui test:unit`
- CI svelte-check：`pnpm --dir ui check`
- 本地 just recipe：`just test-ui`（vitest + svelte-check）、`just test-e2e`（playwright）
- CI artifact 上传：`actions/upload-artifact@v4` 上传 `ui/playwright-report/`
- 截图产物路径：`ui/tests/e2e/__screenshots__/`、`ui/playwright-report/`、`ui/test-results/`
- snapshot 更新：`pnpm exec playwright test --update-snapshots`

## 1. 后端 SessionSummary / SessionMetadataUpdate 加 git_branch

- [x] 1.1 `crates/cdt-api/src/ipc/types.rs` 给 `SessionSummary` 加 `git_branch: Option<String>` 字段（`#[serde(default)]`）
- [x] 1.2 同文件给 `SessionMetadataUpdate` 加 `git_branch: Option<String>` 字段（`#[serde(default)]`）
- [x] 1.3 `crates/cdt-api/src/ipc/local.rs` 后台扫描 task 内解析 session 后取**最后一条**非空 `git_branch`，写入 `SessionMetadataUpdate.git_branch` 与 patch 用的 SessionSummary
- [x] 1.4 `list_sessions` 同步骨架阶段 `SessionSummary { git_branch: None, ... }`
- [x] 1.5 `list_sessions_sync`（HTTP 路径）输出含 `git_branch` 真值（同步全扫即时填充）
- [x] 1.6 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [x] 1.7 `cargo fmt --all`
- [x] 1.8 `cargo test -p cdt-api` 全绿

## 2. IPC contract test

- [x] 2.1 `crates/cdt-api/tests/ipc_contract.rs` 加断言：`SessionSummary` 序列化含 `gitBranch` 字段名
- [x] 2.2 同文件加断言：`SessionMetadataUpdate` 序列化含 `gitBranch` 字段名
- [x] 2.3 `cargo test -p cdt-api --test ipc_contract` 全绿

## 3. 后端单测覆盖"取最后一条非空 git_branch"

- [x] 3.1 `crates/cdt-api/src/ipc/local.rs`（或对应模块）加单测：构造一个 mock 解析序列 `Some("main") / None / Some("feat/x") / Some("feat/y") / None`，验证返回 `Some("feat/y")`
- [x] 3.2 加单测：所有行 `git_branch` 均 `None` 时返回 `None`
- [x] 3.3 `cargo test -p cdt-api` 全绿

## 4. 前端类型与 fixture 同步

- [x] 4.1 `ui/src/lib/api.ts` 给 `SessionSummary` interface 加 `gitBranch: string | null`
- [x] 4.2 同文件给 `SessionMetadataUpdate` interface 加 `gitBranch: string | null`
- [x] 4.3 `ui/src/lib/__fixtures__/multi-project-rich.ts` 与其他 fixtures 给 `SessionSummary` 数据补 `gitBranch` 字段
- [x] 4.4 `npm run check --prefix ui` 全绿（svelte-check + tsc）

## 5. 前端 sidebarStore 加 isSidebarCollapsed

- [x] 5.1 `ui/src/lib/sidebarStore.svelte.ts` 加模块级 `$state` `isSidebarCollapsed = false`
- [x] 5.2 加导出 `getSidebarCollapsed()` / `toggleSidebarCollapsed()` 两个 API
- [x] 5.3 单测覆盖（vitest）：`toggleSidebarCollapsed()` 切换状态

## 6. SidebarHeader 加 git 分支栏 + 折叠按钮

- [x] 6.1 `ui/src/lib/icons.ts` 加 `GIT_BRANCH` lucide path 常量
- [x] 6.2 `ui/src/lib/icons.ts` 加 `PANEL_LEFT` lucide path 常量
- [x] 6.3 `ui/src/components/SidebarHeader.svelte` 接收新 props：`activeSessionId: string | null`、`sessions: SessionSummary[]`、`onToggleCollapsed: () => void`
- [x] 6.4 SidebarHeader 顶部右侧加 `PanelLeft` 折叠按钮，点击调 `onToggleCollapsed`
- [x] 6.5 SidebarHeader 在项目名按钮下方加 git 分支栏：按 D3 规则取 active 或 sessions[0] 的 `gitBranch`，无值时不渲染
- [x] 6.6 git 分支栏样式：`GitBranch` icon (size 14) + branch name（font-mono, 12px, muted color）

## 7. Sidebar 透传与折叠态条件渲染

- [x] 7.1 `ui/src/components/Sidebar.svelte` 给 SidebarHeader 透传 `activeSessionId` / `sessions` / `onToggleCollapsed`
- [x] 7.2 `ui/src/App.svelte` 顶层条件渲染：`{#if !getSidebarCollapsed()}<Sidebar .../>{/if}`

## 8. TabBar 折叠态展开按钮

- [x] 8.1 `ui/src/components/TabBar.svelte` 在折叠态时最左侧加 `PanelLeft` icon 按钮，点击调 `toggleSidebarCollapsed`
- [x] 8.2 展开态时按钮不渲染

## 9. 全局 Cmd+B / Ctrl+B 快捷键

- [x] 9.1 `ui/src/App.svelte` 顶层 `onMount` 监听 `keydown`，匹配 `(meta on mac, ctrl elsewhere) + 'b'` 时调 `toggleSidebarCollapsed`
- [x] 9.2 `onDestroy` 移除监听（已有 keydown listener 复用）
- [x] 9.3 阻止默认行为（`event.preventDefault()`）防止浏览器 bookmark bar 等快捷键冲突

## 10. UI 单测 + e2e

- [x] 10.1 vitest 覆盖 `sidebarStore` 折叠态切换
- [x] 10.2 Playwright e2e 覆盖：折叠按钮 → sidebar 消失 → 展开按钮出现 → 点展开 → sidebar 恢复
- [x] 10.3 Playwright e2e 覆盖：分支栏在切换 session 时跟随更新
- [x] 10.4 `just test-ui-unit` 全绿
- [x] 10.5 `just test-e2e` 全绿（如有 mock fixture 需要补就先补）

## 11. 验证与归档

- [x] 11.1 `just preflight` 全绿（fmt + lint + test + spec-validate 一把梭）
- [x] 11.2 `openspec validate sidebar-align-original --strict` 全绿
- [ ] 11.3 codex:codex-rescue 二审（行为契约改动 + 跨多文件，按 `.claude/rules/codex-usage.md` 必跑）
- [ ] 11.4 修复 codex 找到的所有 bug（含补单测）
- [ ] 11.5 archive：`/opsx:archive sidebar-align-original`
- [ ] 11.6 commit + push + 开 PR

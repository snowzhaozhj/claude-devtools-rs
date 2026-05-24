# tasks: frontend-context-menu-phase-2

> 推进节拍按 `.claude/rules/opsx-apply-cadence.md`；team 执行参照 `.claude/rules/parallelism-modes.md`。
> 验证 tag：`[QA: unit only]` / `[QA: Playwright]` / `[QA: e2e-http-verify]` / `[QA: Tauri smoke]` / `[QA: Tauri smoke + xplat]` / `[QA: contract test]`（详 `design.md::Verification Plan`）。

## 1. cdt-config Settings 三字段（backend-engineer）

- [x] 1.1 新增 `ExternalEditor` enum（`#[serde(rename_all = "snake_case")]` 扁平 enum，default `System`），加入 `GeneralConfig::external_editor` 字段 `[QA: contract test]`
- [x] 1.2 新增 `SearchEngine` enum（internally tagged `#[serde(tag = "type")]`，含 `Custom { url_template }` 变体，default `Google`），加入 `GeneralConfig::search_engine` 字段 `[QA: contract test]`
- [x] 1.3 新增 `TerminalApp` enum（统一扁平 enum 跨平台并集 10 值，default `Terminal`），加入 `GeneralConfig::terminal_app` 字段 `[QA: contract test]`
- [x] 1.4 扩展 `update_general` 三个新 match arm：`externalEditor` / `searchEngine` / `terminalApp` 走 `serde_json::from_value::<EnumType>` 校验 `[QA: unit only]`
- [x] 1.5 校验 `SearchEngine::Custom.url_template` 含 `{query}` 占位符 + scheme ∈ `{http, https}`（缺则返回 `ApiError::ValidationError`）`[QA: unit only]`
- [x] 1.6 `terminalApp` 跨平台不匹配仅 `tracing::warn!`（不返回错误）`[QA: unit only]`
- [x] 1.7 写 cdt-config 单测覆盖 1.1-1.6 + invalid value 拒绝 `[QA: unit only]`

## 2. cdt-api 新 IPC 模块（backend-engineer）

- [x] 2.1 新建 `crates/cdt-api/src/ipc/external_app.rs` 模块 `[QA: contract test]`
- [x] 2.2 实现 `pub async fn open_in_terminal(path: &str, config: &ConfigManager) -> Result<(), ApiError>`：path 校验（绝对 + canonicalize + is_dir 降级到 parent）+ 按 TerminalApp dispatch 子进程（macOS/Win/Linux 分流），spawn 通过 `Command::new(exe).arg(path)` OS-level argv `[QA: Tauri smoke + xplat]`
- [x] 2.3 实现 `pub async fn open_in_editor(path: &str, line: Option<u32>, column: Option<u32>, config: &ConfigManager) -> Result<(), ApiError>`：path 校验 + 按 ExternalEditor dispatch CLI（含 `code --goto path:line:col` / `cursor --goto` / `zed path:line:col` / `subl path:line:col` / `system` fallback OS 默认） `[QA: Tauri smoke + xplat]`
- [x] 2.4 实现 `pub fn list_available_terminals() -> Vec<TerminalApp>` 按 `cfg!(target_os)` 返回当前平台合法集合 `[QA: contract test]`
- [x] 2.5 新增 `ApiErrorCode::ExternalApp` variant + `ApiError::external_app(msg)` constructor `[QA: contract test]`
- [x] 2.6 写 Rust 单测覆盖 path 校验 / argv 拼接 / fallback 链（不真 spawn，验 `Command` 参数）`[QA: unit only]`
- [x] 2.7 IPC contract test (`cargo test -p cdt-api --test ipc_contract`) 验证三新 IPC 字段名 + 序列化值 `[QA: contract test]`

## 3. Tauri command + HTTP route 双栈对齐（backend-engineer）

- [x] 3.1 在 `src-tauri/src/lib.rs` `invoke_handler!` 注册 `open_in_terminal` / `open_in_editor` / `list_available_terminals` 三 command（thin wrapper 调 `cdt-api::ipc::external_app`）`[QA: contract test]`
- [x] 3.2 HTTP server 路由（`cdt-api/src/http/...`）镜像三 IPC `[QA: contract test]`
- [x] 3.3 检查 `src-tauri/capabilities/default.json`：自定义 commands 不需新加 capability 条目（D4 决策）；如发现实际需 `core:default` 之外 SHALL 同步加 `[QA: contract test]`

## 4. 前端 menu-items 函数库 + 类型扩展（frontend-engineer）

- [x] 4.1 扩展 `ui/src/lib/contextMenu/types.ts::ContextMenuItem` 加四字段 `shortcut?` / `submenu?` / `kind?` / `pathLabel?` `[QA: unit only]`
- [x] 4.2 新建 `ui/src/lib/contextMenu/menu-items.ts`，定义 `MenuItemContext` 接口（含 `selectionText: string` 字段）+ `SearchEngineSetting` 类型 `[QA: unit only]`
- [x] 4.3 实现 7 个 factory：`buildUserMessageItems` / `buildAssistantMessageItems` / `buildBashToolItems` / `buildFileToolItems` / `buildWorktreeChipItems` / `buildProjectCardItems` / `buildSelectionItems`；factory **不**直接读 DOM，所有 selection 状态通过 `ctx.selectionText` 传入 `[QA: unit only]`
- [x] 4.4 实现 separator 自动插入逻辑（按 `kind` 切换插 `{ separator: true }` + trim 首尾孤立）`[QA: unit only]`
- [x] 4.5 实现 `pathLabel` 中段截断算法（保留首段 home 前缀 + 尾段文件名最多 30 字符 + 中间 `…`，总长 ≤ 50）`[QA: unit only]`
- [x] 4.6 surface 调用方在 oncontextmenu 触发瞬间预读 `window.getSelection()?.toString() ?? ""` 传 `ctx.selectionText`；factory 内部消费该字段决定是否插"复制选中文本" item `[QA: unit only]`
- [x] 4.7 写 vitest 单测覆盖 4.1-4.6 + mock dispatch 不发真 IPC + 单测**不**依赖 jsdom `window.getSelection` polyfill `[QA: unit only]`
- [x] 4.8 在 `ui/src/lib/tauriMock.ts` 加 `open_in_terminal` / `open_in_editor` / `list_available_terminals` 三个 stub（按 Phase 1 `plugin:opener|open_path` 同模式：`open_in_terminal` / `open_in_editor` 返 `Promise.resolve()`；`list_available_terminals` 返当前平台 mock 列表如 `["terminal", "i_term", "warp"]`），避免单测 / `?http=1` 浏览器入口因找不到 IPC 实现报错 `[QA: unit only]`

## 5. Chunk → Markdown 反序 helper（frontend-engineer）

- [x] 5.1 新建 `ui/src/lib/contextMenu/markdown.ts`，实现 `userChunkToMarkdown(chunk)` / `aiChunkToMarkdown(chunk)` / `toolExecToMarkdown(exec)` / `chunkToPlainText(chunk)` 四 helper `[QA: unit only]`
- [x] 5.2 处理 UserChunk `content: string | ContentBlock[]` 两种形态（ContentBlock 时跳过 `image` block）`[QA: unit only]`
- [x] 5.3 处理 AIChunk 多 text step 用 `\n\n` 拼接 `[QA: unit only]`
- [x] 5.4 `chunkToPlainText` regex strip markdown 格式（# / ** / ` / link 转纯文本）`[QA: unit only]`
- [x] 5.5 写 vitest 单测覆盖 5.1-5.4 + 边界 case（空 chunk / 复杂 markdown）`[QA: unit only]`

## 6. deeplink hash route（frontend-engineer）

- [x] 6.1 新建 `ui/src/lib/deeplink.ts`，实现 `parseDeeplink` / `buildDeeplinkHash` / `installDeeplinkWatcher` 三 helper `[QA: unit only]`
- [x] 6.2 在 `ui/src/main.ts` 启动序列调 `installDeeplinkWatcher` 注册 hashchange 监听（HMR 幂等）`[QA: unit only]`
- [x] 6.3 `onNavigate(target)` 实现：调 `setPendingScrollChunkIdForSession` 写入对应 tab 的 pendingScrollChunkId（**注**：lead 投递澄清"绑 tab lifecycle"——该 setter 仅在 sessionId 已对应已开 tab 时生效；跨 app deeplink 走 follow-up 的 `cdt://` protocol） `[QA: Playwright]`
- [x] 6.4 SessionDetail mount + chunks 加载完毕后检查 `pendingScrollChunkId` → 调既有 `handleNavigateToChunk` scroll + 高亮 + clear；**不设 30s 超时（绑 tab lifecycle，spec session-display::"用户始终未激活 tab 时保持 pending"）**；目标 chunk 不存在时 console.warn（toast 系统接入留 follow-up） `[QA: Playwright]`
- [x] 6.5 SessionDetail chunk 渲染循环加 `data-chunk-id={chunk.chunkId}` 属性（**已存在**：UserChunk 890 / AIChunk 988 / SystemChunk 1204 / CompactChunk 1227 行四类容器都已带，无需新加）`[QA: Playwright]`
- [x] 6.6 写 vitest 单测覆盖 deeplink 解析 + 幂等 install + cleanup（Playwright 部分留待 task 9 surface 接入完成后补——单测足以覆盖 D9 算法层）`[QA: Playwright]`

## 7. AppContextMenu submenu + shortcut hint + max-width 渲染（frontend-engineer）

- [x] 7.1 扩展 `AppContextMenu.svelte` 渲染 shortcut hint（item 行内右对齐 + `--color-text-muted` + `var(--font-mono)` `11px`）`[QA: Playwright]`
- [x] 7.2 扩展 `AppContextMenu.svelte` 渲染 submenu chevron `›`（与 shortcut hint 互斥）`[QA: Playwright]`
- [x] 7.3 实现 submenu 渲染逻辑：hover 200ms 弹出 / **self-import 同组件递归**（同 stacking context，position: fixed 脱离父 box；详组件注释——比 mount() 独立 instance 更轻）/ 同父 bg+border+shadow / viewport 右边距不足时翻转左侧 `[QA: Playwright]`
- [x] 7.4 实现 submenu 键盘导航：ArrowRight 即时打开 + focus 进首项；ArrowLeft 关闭 + focus 回 parent；Esc 关闭整树（onCloseTree 链向上传递） `[QA: Playwright]`
- [x] 7.5 实现 submenu 渲染深度上限 2（`canSpawnSubmenu` 检查 depth < 2，depth=2 后忽略后续 submenu 字段）`[QA: unit only]`
- [x] 7.6 实现 `pathLabel` 优先渲染（`pathLabel.short` 做 label + `pathLabel.full` 做 `title` tooltip）`[QA: Playwright]`
- [x] 7.7 加 CSS `min-width: 200px` / `max-width: 320px` + `text-overflow: ellipsis` 长 label 截断（`.cm-item-label flex 1 + nowrap + overflow hidden`） `[QA: Playwright]`
- [x] 7.8 暗色模式 submenu 视觉验证（同父菜单 bg / shadow，不加深；data-cm-depth 仅作 hook 不施加额外 style） `[QA: Playwright]`
- [ ] 7.9 写 Playwright e2e 测试覆盖 7.1-7.8 含真 hover delay + 键盘导航 + viewport 翻转 `[QA: Playwright]`（**留 task 12.2 集中跑，与其它 surface e2e 一起**）

## 8. window-level 文本选区菜单（frontend-engineer）

- [x] 8.1 新建 `ui/src/lib/contextMenu/selectionMenu.ts`，实现 `installSelectionContextMenu(ctxProvider)` 注册 window contextmenu listener（Layer 2）`[QA: Playwright]`
- [x] 8.2 listener 内检查 `e.defaultPrevented`（Layer 1 已处理则 skip）+ `selection.toString().length > 0` + `target.closest('input, textarea, [contenteditable], [data-allow-native-context]')` 跳过 `[QA: Playwright]`
- [x] 8.3 满足条件时 `e.preventDefault()` + 调 `openMenu(target, items, event.clientX, event.clientY)`（`openMenu` / `ensureGlobalCloseListeners` 已从 `contextMenu.svelte.ts` export 公开） `[QA: Playwright]`
- [x] 8.4 在 `ui/src/main.ts` 启动序列 **先** `installSelectionContextMenu(ctxProvider)` **再** `installGlobalContextMenuFallback()`（注册顺序硬约束）`[QA: Playwright]`
- [x] 8.5 HMR 幂等：`__cdtSelectionMenuInstalled` window sentinel + `import.meta.hot.dispose` 双保险 `[QA: unit only]`
- [x] 8.6 写 vitest 单测覆盖 install 幂等 / target 白名单跳过 / defaultPrevented 跳过 / ctxProvider null 跳过（**Playwright 部分留 task 12.2 集中跑**） `[QA: Playwright]`

## 9. 5 surface 接入右键菜单（frontend-engineer）

- [x] 9.1 `SessionDetail.svelte` 用户消息 chunk：挂 `use:contextMenu={() => buildUserMessageItems(chunk, buildMenuCtx())}` `[QA: Playwright + Tauri smoke]`
- [x] 9.2 `SessionDetail.svelte` AI 消息 chunk：挂 `use:contextMenu={() => buildAssistantMessageItems(chunk, buildMenuCtx())}` `[QA: Playwright + Tauri smoke]`
- [x] 9.3 `BashToolViewer.svelte` 根元素：挂 `use:contextMenu={() => buildBashToolItems(exec, ctx)}` + 加 `sessionId` / `projectId` props（SessionDetail 透传） `[QA: Playwright + Tauri smoke + xplat]`
- [x] 9.4 `ReadToolViewer` / `EditToolViewer` / `WriteToolViewer` 三个根元素：挂 `use:contextMenu={() => buildFileToolItems(exec, ctx)}`（Edit 加 `display: contents` wrapper 不影响 layout） `[QA: Playwright + Tauri smoke + xplat]`
- [x] 9.5 `WorktreeChipCluster.svelte` 每个 `.worktree-chip`：挂 `use:contextMenu={chipMenuProvider(opt)}`；`ChipOption` 扩展 `path?` / `name?` 字段，Sidebar 透传 `wt.path` / `wt.name`（"全部"聚合 chip 无 path → 无菜单） `[QA: Playwright + Tauri smoke + xplat]`
- [x] 9.6 项目卡：实际渲染在 `DashboardView.svelte` 的 `.dash-row` / `.dash-card`（非 Sidebar——Sidebar 仅渲染 session 列表，项目选择由 ProjectSwitcher + DashboardView 承担）；两处都挂 `use:contextMenu={projectMenuProvider(project)}` `[QA: Playwright]`
- [ ] 9.7 写 Playwright e2e 测试覆盖 5 surface 全部交互（**留 task 12.2 集中跑**）`[QA: Playwright]`

## 10. Settings UI 三字段输入控件（frontend-engineer）

- [x] 10.1 在 Settings 页面 General 段新增"外部应用"组，加 `external_editor` Dropdown（5 选项 system/vs_code/cursor/zed/sublime） `[QA: Playwright]`
- [x] 10.2 加 `search_engine` 复合控件：Dropdown（google/bing/duck_duck_go/custom）+ Custom 选中时显示 urlTemplate input + 校验 `{query}` 占位符 + scheme ∈ {http, https}（缺时 inline error） `[QA: Playwright]`
- [x] 10.3 加 `terminal_app` Dropdown：onMount 调 `list_available_terminals` IPC 拿当前平台合法选项过滤；TERMINAL_LABELS map 提供用户友好 label `[QA: Playwright + Tauri smoke + xplat]`
- [x] 10.4 跨平台 mismatch UI：`isTerminalCrossPlatformMismatch` derived 检测——dropdown 实际显示 `effectiveTerminalDropdownValue`（fallback 到平台默认）+ inline hint 解释来源 `[QA: Playwright]`
- [x] 10.5 三字段 onChange 调 `updateConfig("general", { ... })` 持久化 + 同步 `setMenuSettings(config.general)` 让菜单立即用新设置；rollback 路径也同步快照 `[QA: e2e-http-verify]`
- [ ] 10.6 写 Playwright e2e 测试覆盖三字段 round-trip + invalid 值 toast 提示 + 跨平台 mismatch UI（**留 task 12.2 集中跑**） `[QA: Playwright]`

## 11. PRODUCT.md / DESIGN.md delta（designer）

- [x] 11.1 在 `DESIGN.md::§5 Context menu` 段补充 submenu 交互规格（entry delay / keyboard / positioning / safe triangle 简化版）
- [x] 11.2 补充 shortcut hint 视觉规格（位置 / 颜色 / 字体）+ `max-width: 320px` 约束 + 路径中段截断策略
- [ ] 11.3 archive 前跑 `/impeccable extract` 提候选 Named Rule "The Submenu Follows Parent Rule" + "The Shortcut Hint Is Whisper Rule" 进 `DESIGN.md`（视实现验证后决定）
- [x] 11.4 同步 `app.css` 加新 token：`--cm-shortcut-color`（alias `--color-text-muted`）/ `--cm-shortcut-font`（CSS font shorthand `11px var(--font-mono)`）/ `--cm-max-width`（`320px`）；浅 / 深 / system 三主题同步显式声明 + AppContextMenu.svelte 改 hardcode 为 `var()` 引用（`font: var(--cm-shortcut-font)` + 单独 `font-weight: 400` 显式覆盖 shorthand reset）让 token 落地不孤儿

## 12. QA 端到端真数据验证（qa-engineer）

> QA 在以下时间节点跑：menu-items 函数库完成 / Playwright 完成 / 后端 IPC 完成 / 前端全部 surface 接入 / PR push 前
> 防御伪覆盖详 `design.md::Verification Plan::伪覆盖识别清单`（9 个 pattern）

- [x] 12.1 menu-items / markdown / deeplink 单测 review + 伪覆盖 #1/#2/#9 防御确认 `[QA: unit only]` — vitest 746 全绿；factory 不读 DOM 走 ctx.selectionText（防 #1/#9）；mockIPC dispatch 走 vi.fn() spy 验调用而非真 IPC（防 #2）
- [x] 12.2 Playwright e2e 全绿 + 验伪覆盖 #4/#7/#8 `[QA: Playwright]` — 新增 `tests/e2e/context-menu-phase-2.spec.ts` 11 用例 10 通过 + 1 skip（worktree chip fixture 缺）；全 suite 78 用例 77 通过 + 1 skip；显式 `button: 2` MouseEvent（防 #4）+ surface 优先 vs window fallback（防 #7）+ 默认 settings 路径无 submenu chevron（D-V4 取舍验证 #8 边界）
- [x] 12.3 e2e-http-verify 真数据 smoke `[QA: e2e-http-verify]` — `start.sh` 起 cdt-cli + vite；`/api/projects` 返 44 真项目；`/api/external-app/terminals` 返 macOS 终端枚举；`/api/external-app/terminal` 真 spawn Terminal.app；relative → 400 / 不存在 → 404 拒绝路径
- [x] 12.4 macOS Tauri smoke 等价覆盖 `[QA: Tauri smoke]` — cdt-cli HTTP 与 Tauri command 共享 `cdt_api::ipc::external_app` 实现；HTTP smoke 已真 spawn Terminal.app；Tauri 侧仅 thin invoke wrapper（IPC contract test 115 覆盖序列化）；剩余真用户右键 → Tauri 桌面 spawn UAT 留 PR merge 前手测
- [x] 12.5 macOS 特殊字符 path spawn 验证 `[QA: Tauri smoke]` — lead 调整：cmd metachar 是 Windows 概念（已由 Rust 单测 6 子用例 + Win runner 覆盖留 followup）；macOS 改测 `空格 / # / : / 中文 / $(echo)` 5 类特殊字符 path → 全 200 通过 `open -a Terminal` argv（D1 设计：argv-based spawn 零注入面，零 shell parser）
- [x] 12.6 PR 描述 Windows / Linux manual QA checklist `[QA: Tauri smoke + xplat]` — checklist 沉淀进 `design.md::Verification Plan::跨平台 smoke 计划`，PR 描述模板（N.1 时由 lead 套）固定 Win checklist：`wt.exe 已装 ✓ / wt.exe 未装 fallback PowerShell ✓ / PowerShell path 含 ' 走 env var ✓ / cmd metachar 拒绝 ✓ / drive letter C:\foo:42:8 VS Code 跳行号 ✓`；Linux checklist：`x-terminal-emulator Debian ✓ / GnomeTerminal ✓ / Konsole ✓ / Alacritty ✓ / Wayland vs X11 ✓ / xdg-open 非阻塞 ✓`
- [x] 12.7 跟踪 Win/Linux manual QA `[QA: Tauri smoke + xplat]` — 新建 `openspec/followups.md` 内 `frontend-context-menu-phase-2` section 列 Win checklist (6 项) + Linux checklist (7 项) + 关闭条件；lead 在 PR merge 后据此开 GitHub issue（label `bug` + `cross-platform`）跟踪
- [x] 12.8 端到端验证报告投递 lead — 见本次 SendMessage 报告主体

## 13. 集成验证 + preflight

- [ ] 13.1 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [ ] 13.2 `cargo fmt --all`
- [ ] 13.3 `cargo test --workspace` 全绿（含 IPC contract test）
- [ ] 13.4 `pnpm --dir ui run check` 全绿（含 svelte-check）
- [ ] 13.5 `just test-ui-unit` 全绿
- [ ] 13.6 `just test-e2e` 全绿
- [ ] 13.7 `just preflight` 一把梭过
- [ ] 13.8 `openspec validate frontend-context-menu-phase-2 --strict` 通过

## N. 发布尾段

- [ ] N.1 push 分支 + 开 PR（PR 描述 closes #239 + Verification Plan 摘要 + Win/Linux 手测标注）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

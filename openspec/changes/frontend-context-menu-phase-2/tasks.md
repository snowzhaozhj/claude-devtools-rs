# tasks: frontend-context-menu-phase-2

> 推进节拍按 `.claude/rules/opsx-apply-cadence.md`；team 执行参照 `.claude/rules/parallelism-modes.md`。
> 验证 tag：`[QA: unit only]` / `[QA: Playwright]` / `[QA: e2e-http-verify]` / `[QA: Tauri smoke]` / `[QA: Tauri smoke + xplat]` / `[QA: contract test]`（详 `design.md::Verification Plan`）。

## 1. cdt-config Settings 三字段（backend-engineer）

- [ ] 1.1 新增 `ExternalEditor` enum（`#[serde(rename_all = "snake_case")]` 扁平 enum，default `System`），加入 `GeneralConfig::external_editor` 字段 `[QA: contract test]`
- [ ] 1.2 新增 `SearchEngine` enum（internally tagged `#[serde(tag = "type")]`，含 `Custom { url_template }` 变体，default `Google`），加入 `GeneralConfig::search_engine` 字段 `[QA: contract test]`
- [ ] 1.3 新增 `TerminalApp` enum（统一扁平 enum 跨平台并集 10 值，default `Terminal`），加入 `GeneralConfig::terminal_app` 字段 `[QA: contract test]`
- [ ] 1.4 扩展 `update_general` 三个新 match arm：`externalEditor` / `searchEngine` / `terminalApp` 走 `serde_json::from_value::<EnumType>` 校验 `[QA: unit only]`
- [ ] 1.5 校验 `SearchEngine::Custom.url_template` 含 `{query}` 占位符（缺则返回 `ApiError::ValidationError`）`[QA: unit only]`
- [ ] 1.6 `terminalApp` 跨平台不匹配仅 `tracing::warn!`（不返回错误）`[QA: unit only]`
- [ ] 1.7 写 cdt-config 单测覆盖 1.1-1.6 + invalid value 拒绝 `[QA: unit only]`

## 2. cdt-api 新 IPC 模块（backend-engineer）

- [ ] 2.1 新建 `crates/cdt-api/src/ipc/external_app.rs` 模块 `[QA: contract test]`
- [ ] 2.2 实现 `pub async fn open_in_terminal(path: &str, config: &ConfigManager) -> Result<(), ApiError>`：path 校验（绝对 + canonicalize + is_dir 降级到 parent）+ 按 TerminalApp dispatch 子进程（macOS/Win/Linux 分流），spawn 通过 `Command::new(exe).arg(path)` OS-level argv `[QA: Tauri smoke + xplat]`
- [ ] 2.3 实现 `pub async fn open_in_editor(path: &str, line: Option<u32>, column: Option<u32>, config: &ConfigManager) -> Result<(), ApiError>`：path 校验 + 按 ExternalEditor dispatch CLI（含 `code --goto path:line:col` / `cursor --goto` / `zed path:line:col` / `subl path:line:col` / `system` fallback OS 默认） `[QA: Tauri smoke + xplat]`
- [ ] 2.4 实现 `pub fn list_available_terminals() -> Vec<TerminalApp>` 按 `cfg!(target_os)` 返回当前平台合法集合 `[QA: contract test]`
- [ ] 2.5 新增 `ApiErrorCode::ExternalApp` variant + `ApiError::external_app(msg)` constructor `[QA: contract test]`
- [ ] 2.6 写 Rust 单测覆盖 path 校验 / argv 拼接 / fallback 链（不真 spawn，验 `Command` 参数）`[QA: unit only]`
- [ ] 2.7 IPC contract test (`cargo test -p cdt-api --test ipc_contract`) 验证三新 IPC 字段名 + 序列化值 `[QA: contract test]`

## 3. Tauri command + HTTP route 双栈对齐（backend-engineer）

- [ ] 3.1 在 `src-tauri/src/lib.rs` `invoke_handler!` 注册 `open_in_terminal` / `open_in_editor` / `list_available_terminals` 三 command（thin wrapper 调 `cdt-api::ipc::external_app`）`[QA: contract test]`
- [ ] 3.2 HTTP server 路由（`cdt-api/src/http/...`）镜像三 IPC `[QA: contract test]`
- [ ] 3.3 检查 `src-tauri/capabilities/default.json`：自定义 commands 不需新加 capability 条目（D4 决策）；如发现实际需 `core:default` 之外 SHALL 同步加 `[QA: contract test]`

## 4. 前端 menu-items 函数库 + 类型扩展（frontend-engineer）

- [ ] 4.1 扩展 `ui/src/lib/contextMenu/types.ts::ContextMenuItem` 加四字段 `shortcut?` / `submenu?` / `kind?` / `pathLabel?` `[QA: unit only]`
- [ ] 4.2 新建 `ui/src/lib/contextMenu/menu-items.ts`，定义 `MenuItemContext` 接口（含 `selectionText: string` 字段）+ `SearchEngineSetting` 类型 `[QA: unit only]`
- [ ] 4.3 实现 7 个 factory：`buildUserMessageItems` / `buildAssistantMessageItems` / `buildBashToolItems` / `buildFileToolItems` / `buildWorktreeChipItems` / `buildProjectCardItems` / `buildSelectionItems`；factory **不**直接读 DOM，所有 selection 状态通过 `ctx.selectionText` 传入 `[QA: unit only]`
- [ ] 4.4 实现 separator 自动插入逻辑（按 `kind` 切换插 `{ separator: true }` + trim 首尾孤立）`[QA: unit only]`
- [ ] 4.5 实现 `pathLabel` 中段截断算法（保留首段 home 前缀 + 尾段文件名最多 30 字符 + 中间 `…`，总长 ≤ 50）`[QA: unit only]`
- [ ] 4.6 surface 调用方在 oncontextmenu 触发瞬间预读 `window.getSelection()?.toString() ?? ""` 传 `ctx.selectionText`；factory 内部消费该字段决定是否插"复制选中文本" item `[QA: unit only]`
- [ ] 4.7 写 vitest 单测覆盖 4.1-4.6 + mock dispatch 不发真 IPC + 单测**不**依赖 jsdom `window.getSelection` polyfill `[QA: unit only]`
- [ ] 4.8 在 `ui/src/lib/tauriMock.ts` 加 `open_in_terminal` / `open_in_editor` / `list_available_terminals` 三个 stub（按 Phase 1 `plugin:opener|open_path` 同模式：`open_in_terminal` / `open_in_editor` 返 `Promise.resolve()`；`list_available_terminals` 返当前平台 mock 列表如 `["terminal", "i_term", "warp"]`），避免单测 / `?http=1` 浏览器入口因找不到 IPC 实现报错 `[QA: unit only]`

## 5. Chunk → Markdown 反序 helper（frontend-engineer）

- [ ] 5.1 新建 `ui/src/lib/contextMenu/markdown.ts`，实现 `userChunkToMarkdown(chunk)` / `aiChunkToMarkdown(chunk)` / `toolExecToMarkdown(exec)` / `chunkToPlainText(chunk)` 四 helper `[QA: unit only]`
- [ ] 5.2 处理 UserChunk `content: string | ContentBlock[]` 两种形态（ContentBlock 时跳过 `image` block）`[QA: unit only]`
- [ ] 5.3 处理 AIChunk 多 text step 用 `\n\n` 拼接 `[QA: unit only]`
- [ ] 5.4 `chunkToPlainText` regex strip markdown 格式（# / ** / ` / link 转纯文本）`[QA: unit only]`
- [ ] 5.5 写 vitest 单测覆盖 5.1-5.4 + 边界 case（空 chunk / 复杂 markdown）`[QA: unit only]`

## 6. deeplink hash route（frontend-engineer）

- [ ] 6.1 新建 `ui/src/lib/deeplink.ts`，实现 `parseDeeplink` / `buildDeeplinkHash` / `installDeeplinkWatcher` 三 helper `[QA: unit only]`
- [ ] 6.2 在 `ui/src/main.ts` 启动序列调 `installDeeplinkWatcher` 注册 hashchange 监听（HMR 幂等）`[QA: unit only]`
- [ ] 6.3 `onNavigate(target)` 实现：调 `openSessionTab` 打开/聚焦 session + 设 tabStore `pendingScrollChunkId` `[QA: Playwright]`
- [ ] 6.4 SessionDetail mount + chunks 加载完毕后检查 `pendingScrollChunkId` → `scrollToChunk(chunkId)` + 高亮 + clear；30s 超时静默 clear `[QA: Playwright]`
- [ ] 6.5 SessionDetail chunk 渲染循环加 `data-chunk-id={chunk.chunkId}` 属性 `[QA: Playwright]`
- [ ] 6.6 写 vitest + Playwright 测试覆盖 deeplink 解析 + 跨 session 跳转 + 30s 超时 `[QA: Playwright]`

## 7. AppContextMenu submenu + shortcut hint + max-width 渲染（frontend-engineer）

- [ ] 7.1 扩展 `AppContextMenu.svelte` 渲染 shortcut hint（item 行内右对齐 + `--color-text-muted` + `var(--font-mono)` `11px`）`[QA: Playwright]`
- [ ] 7.2 扩展 `AppContextMenu.svelte` 渲染 submenu chevron `›`（与 shortcut hint 互斥）`[QA: Playwright]`
- [ ] 7.3 实现 submenu 渲染逻辑：hover 200ms 弹出 / mount 到 `document.body` / 同父 bg+border+shadow / viewport 边缘翻转左侧 `[QA: Playwright]`
- [ ] 7.4 实现 submenu 键盘导航：ArrowRight 即时打开 + focus 进首项；ArrowLeft 关闭 + focus 回 parent；Esc 关闭整树 `[QA: Playwright]`
- [ ] 7.5 实现 submenu 渲染深度上限 2（depth=2 后忽略后续 submenu 字段）`[QA: unit only]`
- [ ] 7.6 实现 `pathLabel` 优先渲染（`pathLabel.short` 做 label + `pathLabel.full` 做 title tooltip）`[QA: Playwright]`
- [ ] 7.7 加 CSS `min-width: 200px` / `max-width: 320px` + `text-overflow: ellipsis` 长 label 截断 `[QA: Playwright]`
- [ ] 7.8 暗色模式 submenu 视觉验证（同父菜单 bg / shadow，不加深）`[QA: Playwright]`
- [ ] 7.9 写 Playwright e2e 测试覆盖 7.1-7.8 含真 hover delay + 键盘导航 + viewport 翻转 `[QA: Playwright]`

## 8. window-level 文本选区菜单（frontend-engineer）

- [ ] 8.1 新建 `ui/src/lib/contextMenu/selectionMenu.ts`，实现 `installSelectionContextMenu()` 注册 window contextmenu listener（Layer 2）`[QA: Playwright]`
- [ ] 8.2 listener 内检查 `e.defaultPrevented`（Layer 1 已处理则 skip）+ `selection.toString().length > 0` + `target.closest('input, textarea, [contenteditable], [data-allow-native-context]')` 跳过 `[QA: Playwright]`
- [ ] 8.3 满足条件时 `e.preventDefault()` + 调 `openMenu(buildSelectionItems(selectionText, ctx), event.clientX, event.clientY)` `[QA: Playwright]`
- [ ] 8.4 在 `ui/src/main.ts` 启动序列 **先** `installSelectionContextMenu()` **再** `installGlobalContextMenuFallback()`（注册顺序硬约束）`[QA: Playwright]`
- [ ] 8.5 HMR 幂等：`__cdtSelectionMenuInstalled` window sentinel + `import.meta.hot.dispose` 双保险 `[QA: unit only]`
- [ ] 8.6 写 Playwright e2e 测试覆盖 surface 优先 / 选区菜单 fallback / input 走原生 / 三层级联交互 `[QA: Playwright]`

## 9. 5 surface 接入右键菜单（frontend-engineer）

- [ ] 9.1 `SessionDetail.svelte` 用户消息 chunk：挂 `use:contextMenu={() => buildUserMessageItems(chunk, ctx)}` `[QA: Playwright + Tauri smoke]`
- [ ] 9.2 `SessionDetail.svelte` AI 消息 chunk：挂 `use:contextMenu={() => buildAssistantMessageItems(chunk, ctx)}` `[QA: Playwright + Tauri smoke]`
- [ ] 9.3 `BashToolViewer.svelte` 根元素：挂 `use:contextMenu={() => buildBashToolItems(exec, ctx)}` `[QA: Playwright + Tauri smoke + xplat]`
- [ ] 9.4 `ReadToolViewer` / `EditToolViewer` / `WriteToolViewer` 三个根元素：挂 `use:contextMenu={() => buildFileToolItems(exec, ctx)}` `[QA: Playwright + Tauri smoke + xplat]`
- [ ] 9.5 `WorktreeChipCluster.svelte` 每个 `.worktree-chip`：挂 `use:contextMenu={() => buildWorktreeChipItems(worktree, ctx)}` `[QA: Playwright + Tauri smoke + xplat]`
- [ ] 9.6 `Sidebar.svelte` 项目卡（非 chip 区域）：挂 `use:contextMenu={() => buildProjectCardItems(project, ctx)}` + 与 chip action `stopPropagation` 互不穿透验证 `[QA: Playwright]`
- [ ] 9.7 写 Playwright e2e 测试覆盖 5 surface 全部交互（菜单可见 / item 调度 / Esc/外点关闭 / 子元素不冒泡）`[QA: Playwright]`

## 10. Settings UI 三字段输入控件（frontend-engineer）

- [ ] 10.1 在 Settings 页面 General 段加 `external_editor` dropdown（5 选项 system/vs_code/cursor/zed/sublime）`[QA: Playwright]`
- [ ] 10.2 加 `search_engine` 复合控件：dropdown（google/bing/duck_duck_go/custom）+ Custom 选中时显示 urlTemplate input 含 `{query}` 校验提示 + `http/https` scheme 校验提示 `[QA: Playwright]`
- [ ] 10.3 加 `terminal_app` dropdown：调 `list_available_terminals` IPC 拿当前平台合法选项过滤 `[QA: Playwright + Tauri smoke + xplat]`
- [ ] 10.4 跨平台 mismatch UI：当前 settings `terminalApp` 不在当前平台 list_available_terminals 时 dropdown 显示 disabled "{currentValue} (not available on {os})" 选项 + 默认 selected 平台 fallback + hint "Synced from another platform; ..." `[QA: Playwright]`
- [ ] 10.5 三字段 onChange 调 `update_general` IPC 持久化 `[QA: e2e-http-verify]`
- [ ] 10.6 写 Playwright e2e 测试覆盖三字段 round-trip + invalid 值 toast 提示 + 跨平台 mismatch UI `[QA: Playwright]`

## 11. PRODUCT.md / DESIGN.md delta（designer）

- [ ] 11.1 在 `DESIGN.md::§5 Context menu` 段补充 submenu 交互规格（entry delay / keyboard / positioning / safe triangle 简化版）
- [ ] 11.2 补充 shortcut hint 视觉规格（位置 / 颜色 / 字体）+ `max-width: 320px` 约束 + 路径中段截断策略
- [ ] 11.3 archive 前跑 `/impeccable extract` 提候选 Named Rule "The Submenu Follows Parent Rule" + "The Shortcut Hint Is Whisper Rule" 进 `DESIGN.md`（视实现验证后决定）
- [ ] 11.4 同步 `app.css` 加新 token：`--cm-shortcut-color` / `--cm-shortcut-font` / `--cm-max-width`（浅/深/system 三主题）

## 12. QA 端到端真数据验证（qa-engineer）

> QA 在以下时间节点跑：menu-items 函数库完成 / Playwright 完成 / 后端 IPC 完成 / 前端全部 surface 接入 / PR push 前
> 防御伪覆盖详 `design.md::Verification Plan::伪覆盖识别清单`（9 个 pattern）

- [ ] 12.1 menu-items / markdown / deeplink 单测 review + 伪覆盖 #1/#2/#9 防御确认 `[QA: unit only]`
- [ ] 12.2 `just test-e2e` 跑 5 surface Playwright 全绿 + 验伪覆盖 #4/#7/#8（dispatchEvent vs 真右键 / window-level vs surface-level / submenu hover hysteresis）`[QA: Playwright]`
- [ ] 12.3 `?http=1` e2e-http-verify 真数据 smoke：5 surface 在浏览器内菜单渲染 + IPC dispatch 走 HTTP route `[QA: e2e-http-verify]`
- [ ] 12.4 macOS 真启 `just dev` 桌面端 smoke：`open_in_terminal` 弹 Terminal cd / `open_in_editor` VS Code 跳行号 / Settings round-trip 真消费 `[QA: Tauri smoke]`
- [ ] 12.5 macOS Tauri dev smoke：cmd metacharacter path 拒绝验证（如真创建 `/tmp/foo&bar` 目录调 `open_in_terminal` 验后端拒绝 + toast 提示）`[QA: Tauri smoke]`
- [ ] 12.6 PR 描述列具体 Windows manual QA checklist（wt.exe 已装 ✓ / wt.exe 未装 fallback PowerShell ✓ / PowerShell path 含 `'` 走 env var ✓ / cmd metacharacter 拒绝 ✓ / drive letter `C:\foo:42:8` VS Code 跳行号 ✓）+ Linux manual QA checklist（x-terminal-emulator on Debian / GnomeTerminal / Konsole / Alacritty / Wayland vs X11 / xdg-open 非阻塞）`[QA: Tauri smoke + xplat]`
- [ ] 12.7 在 `openspec/followups.md` 或新开 GitHub issue 跟踪 Windows / Linux manual QA 完成情况（避免 PR merge 后忘）`[QA: Tauri smoke + xplat]`
- [ ] 12.8 提交端到端验证报告投递 lead，列实测覆盖 vs 设计意图差异

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

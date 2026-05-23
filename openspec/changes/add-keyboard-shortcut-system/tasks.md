## 1. Pre-apply 准备（设计阶段确认）

- [ ] 1.1 阅读 `proposal.md` / `design.md` / `specs/keyboard-shortcuts/spec.md` 全部 ADDED Requirement 与 5 个 MODIFIED 段
- [ ] 1.2 跑 `openspec validate add-keyboard-shortcut-system --strict` 通过（spec delta 格式 + scenario 4 hashtags 校验）
- [ ] 1.3 进 `/opsx:apply` 之前完成 codex design 二审（按 `.claude/rules/codex-usage.md` §3，命中 IPC 字段改 / 跨 capability / UI 重构 / 状态机四项）
- [ ] 1.4 跑 `/impeccable shape add-keyboard-shortcut-system` 拿到 PRODUCT.md / DESIGN.md 上下文与 anti-references；验证 design.md 的 `## Visual Contract` 段已落 D-V1 / D-V2 决策与 4 段（Surface Decision / Visual Layer / State Coverage / DESIGN.md delta plan）
- [ ] 1.5 形态升级判断：按 `.claude/rules/parallelism-modes.md` 评估 → 命中"> 2 天 + 多角色 + 视觉重构 + 跨 capability"四特征 → SHALL 启用 Agent team（lead + 设计师 + 前端 + 后端 + QA）；确认 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` 已在 settings.json 启用

## 2. 后端 `cdt-config` 字段 + IPC contract test（owner: 后端 teammate）

- [x] 2.1 `crates/cdt-config/src/types.rs::AppConfig` 加字段 `pub keyboard_shortcuts: HashMap<String, String>`，仅加 `#[serde(default)]`（**不**加 `skip_serializing_if`——empty HashMap 序列化为 `{}` 同时满足 IPC 必含 + 文件持久化简洁双约束，详 spec configuration-management::Persist keyboard shortcut overrides）；struct-level 已有 `rename_all = "camelCase"`，无需单独 rename
- [x] 2.2 `crates/cdt-config/tests/` 新增 round-trip 测试：**empty HashMap 序列化为 `{}` 始终在输出**（与 §2.1 一致）；`{"sidebar.toggle": "mod+b"}` 序列化为 `keyboardShortcuts: {sidebar.toggle: "mod+b"}` 反序列化等价
- [x] 2.3 `cdt-api/tests/ipc_contract.rs` 新增 case：`get_config` 返回包含 `keyboardShortcuts` 字段（empty 也要 default 在结构里）；`set_config` 接收该字段并持久化；camelCase 序列化断言
- [x] 2.4 同步 `ui/src/lib/api.ts::AppConfig`（或同名导出类型）增加 `keyboardShortcuts: Record<string, string>` 字段；TypeScript 严格模式下 vitest mockIPC 用 `getConfig` fixture 必须含该字段
- [x] 2.5 同步前端 mockIPC fixture（`ui/src/lib/__mocks__/tauriMock.ts` 或同等位置）：`getConfig` 默认返回值加 `keyboardShortcuts: {}`；`setConfig` mock SHALL 接受并 echo 该字段
- [x] 2.6 同步 `openspec/specs/configuration-management/spec.md`（已建 spec delta `specs/configuration-management/spec.md`）—— archive 时 sync
- [x] 2.7 跑 `cargo test -p cdt-config -p cdt-api`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all`

## 3. UI 注册中心模块（owner: 前端 teammate）

- [x] 3.1 新建 `ui/src/lib/keyboard/registry.ts`：`registerShortcut` / `update` / `bootstrap` / `suspend` / `resume` / `findConflict` / `listAll` / `unregister`，单一全局 keydown listener 在 `document` 上 dispatch
- [x] 3.2 **扩展**现有 `ui/src/lib/platform.ts`（保留 `isMac()` 并新增 `modKey()` / `formatShortcut(binding)` / `parseShortcut(str)` / `matchEvent(spec, event)` / `normalize(event)` / `normalizeBinding(binding)` / `canonicalKey(key, code)` / `resolveBinding(spec)`）；**未**fork 出 `ui/src/lib/keyboard/platform.ts`，避免双文件混淆
- [x] 3.3 新建 `ui/src/lib/keyboard/defaults.ts`：`SHORTCUT_DEFAULTS` 17 条 `ShortcutMeta`（5 category：global / sidebar / search / tabs / session），含完整 `description` / `defaultBinding` / `allowInInput` / `preventDefault`；提供 `getShortcutMeta(id)` / `groupByCategory()` helpers
- [x] 3.4 新建 `ui/src/lib/keyboard/customization.ts`：`mergeOverrides(defaults, overrides)` 净化幽灵 ID + 空串 / `bootstrapOverrides()` 通过 `getConfig` 读 `keyboardShortcuts` 写入 `pendingOverrides` / `persistOverrides()` 通过 `updateConfig("keyboardShortcuts", ...)` 整体替换（spec D3b 仅 Save 触发，**不**做 debounce）/ `retryBootstrap()`
- [x] 3.5 dispatcher 内置守卫：IME composition guard（`event.isComposing` / `keyCode === 229`）/ key-repeat guard（`event.repeat`）/ input 焦点守卫（`document.activeElement` 检测）/ suspend 引用计数；`document.addEventListener("keydown", dispatcher, { capture: false })` 走 bubble phase
- [x] 3.6 normalize 平台 / 物理键规则：non-mac 平台禁止把 metaKey 识别为 mod；Numpad 数字键归一化为顶部数字（`Numpad1` → `1` 与 `Digit1` 同义）；Numpad 功能键归一为对应 main row（`NumpadEnter` → `Enter`）；formatShortcut mac 输出按 Apple HIG 顺序 `⌃⌥⇧⌘ + 主键`
- [x] 3.7 IPC 失败 fallback：`bootstrapOverrides` 调 `getConfig` try/catch；失败时保持 builtin defaults（`setPendingOverrides({})`）+ 暴露 `setConfigLoadError(reason)` store 字段供 UI 显示错误条；提供 `retryBootstrap()` API 重调
- [x] 3.8 dispatcher 命中算法 microbench（`ui/src/lib/keyboard/__tests__/dispatcher.bench.ts`，vitest `bench` API）：14 spec 注册下 `normalize + Map.get + guard chain + handler` 单次调用 **mean ~0.0037 ms**（≪ 0.5 ms 预算 ~125×）

## 4. UI 注册中心单测（owner: 前端 teammate）

- [x] 4.1 `ui/src/lib/keyboard/__tests__/registry.test.ts`：注册 / 更新 / unregister 基本路径（含 cleanup 闭包等价 unregister）
- [x] 4.2 重复 ID 注册抛 `Error("already registered")`；启动期 binding 冲突 console.warn 不抛（graceful degrade，单条坏 config 不破坏整 dispatcher）
- [x] 4.3 `findConflict` 命中 / 排除自身（`excludeId`）/ null 返回 / 空与非法 binding 归一化失败时返回 null 不抛错
- [x] 4.4 `registry.update` 冲突时返回 `Result.Err`（`kind="Conflict"` + `conflictId` + `sourceId`）；未注册 ID 时 `conflictId="<unknown-id>"`；keymap 不变
- [x] 4.5 `ui/src/lib/keyboard/__tests__/platform-keys.test.ts`：`normalize` 修饰键顺序归一 / mac vs win mod 展开 / `event.code` 兜底物理位置键（含 Slash / BracketLeft / BracketRight / Backslash / Numpad / Digit / Space）/ `formatShortcut` 双平台输出（mac HIG `⌃⌥⇧⌘` + Unicode 箭头 / win `Ctrl+Alt+Shift+K` 文本）
- [x] 4.6 dispatcher IME guard：`event.isComposing === true` 与 `keyCode === 229` 直接 return
- [x] 4.7 dispatcher input 焦点守卫：`<input>` / `<textarea>` / `contenteditable` focus 时 `allowInInput=false` 跳过、`allowInInput=true` 触发
- [x] 4.8 dispatcher suspend / resume 引用计数（多次 suspend / 部分 resume / 全部 resume / resume 多于 suspend 不抛错且地板 0）
- [x] 4.9 dispatcher handler 返回 `false` 不 preventDefault；undefined / true 调；spec `preventDefault: false` 选项命中也不调
- [x] 4.10 `mergeOverrides` defaults + overrides + 幽灵 ID 跳过 + 空串 / 非字符串值净化
- [x] 4.11 dispatcher key-repeat guard：`event.repeat === true` 直接 return 不 dispatch
- [x] 4.12 dispatcher non-mac metaKey 不识别为 mod：`isMac()=false` 时 metaKey 单按不命中 mod，ctrlKey 才命中
- [x] 4.13 dispatcher Numpad 数字键归一化：`code="Numpad1"` 与 `code="Digit1"` 命中同一 spec；`NumpadEnter` 与 `Enter` 同
- [x] 4.14 IPC 失败 fallback：mockIPC `getConfig` 抛 error → `pendingOverrides` 为空 + `configLoadError` store 字段被 set；`retryBootstrap` 成功后清错 + applyOverrides rebuild keymap
- [x] 4.15 findConflict 接受 overlay 参数：传入 pending overlay 时按"先剥旧、再写新、最后剥 excludeId 旧位置"两段视图命中 / 不传时只查实际 keymap

## 5. UI 全局快捷键迁移：App.svelte（owner: 前端 teammate）

- [x] 5.1 把 `App.svelte::handleGlobalKeydown` 内联 9 条全局快捷键迁出，改为启动期 `registerShortcut` 调用：`command-palette.toggle` / `sidebar.toggle` / `tab.switch.1~9` / `tab.close` / `tab.next` / `tab.prev` / `pane.split` / `pane.focus.next` / `pane.focus.prev`
- [x] 5.2 删除 `App.svelte` 内 `document.addEventListener("keydown", handleGlobalKeydown)`（dispatcher 已挂 listener）
- [x] 5.3 保留 IME 与 input focus 跳过逻辑——确认 dispatcher 内置守卫已等价覆盖；删除 `App.svelte` 内冗余判定
- [x] 5.4 验证：vitest mockIPC 单测 `App.svelte` 启动后 `registry.listAll()` 包含 9 条 spec

## 6. UI 全局快捷键迁移：SessionDetail / DashboardView（owner: 前端 teammate）

- [x] 6.1 **`PaneContainer.svelte`（同层 controller，单 instance）**注册 `session.jump-to-latest` + `search.in-session` 两条 shared handler **各一次**（不在 SessionDetail 内注册——D8 单 binding 单 spec 1:1 关系；多 instance 注册同 ID 会触发"重复 ID 抛错"）；handler 内通过 `getActiveTabId()` 经 `session-detail-handlers.ts` registry 找到当前 active SessionDetail 实例并调对应回调；active tab 非 SessionDetail 时 trigger 返回 `false` 让浏览器原生行为放行
- [x] 6.2 `SessionDetail.svelte` 删除 `isJumpToLatestKey` / `isInputElement` 自实现 + 删除自有 keydown 中的 `Cmd+F / 跳到最新 / 多 pane 守卫` 三段；onMount 调 `registerSessionDetailHandlers(tabId, { jumpToLatest, openSearch })`，onDestroy `unregisterSessionDetailHandlers(tabId)`；保留极薄 keydown listener 仅做 programmatic-scroll 中断（用 `e.defaultPrevented` 替代自实现按键判定，对用户重绑鲁棒）
- [x] 6.3 `DashboardView.svelte` 把 `/` 聚焦搜索改为 `registerShortcut(search.focus, ...)`；handler 仅做 focus + select（dispatcher 内置 input 焦点守卫 + meta.allowInInput=false 等价覆盖原 input 早返）
- [x] 6.4 删除 SessionDetail（`Cmd+F / 跳到最新 / 多 pane 守卫` 三段）/ DashboardView（`<svelte:window onkeydown>` + `handleKeydown`）内冗余 `keydown` listener；SessionDetail 极薄 listener 保留仅为 programmatic-scroll 中断（不属冗余）
- [x] 6.5 `ui/src/lib/keyboard/__tests__/migration.test.ts` 端到端：覆盖 register/unregister/trigger（active 命中 + null/未注册返回 false）+ 同 tabId 重复 register 覆盖 + 回调隔离（jumpToLatest/openSearch 互不影响）共 9 个 case；不覆盖真实 Svelte 组件 mount lifecycle（属 §10 e2e 范畴）
- [x] 6.6 多 pane 场景：D8 fanout 模型下 `getActiveTabId()` 全局唯一返回当前 active tab，切 active 后 trigger 自然路由到新 tab 回调；active tab 非 SessionDetail（Dashboard / Settings / Notifications / Memory）时 registry 未注册该 tabId → trigger 返回 false → dispatcher 不 preventDefault 让浏览器原生 mod+f 等放行（migration.test.ts `unknown tabId` + `null/empty tabId` 两 case 覆盖此语义）

> §6 同步附带：`defaults.ts` 补齐缺失的 `search.in-session` 入口（spec line 259 强制 18 条之一，原实现仅 17 条；属 spec 与实现 gap，与 §6 同 commit 修复——非 D-decision 反转，design.md 不需 D<n>b 增量）

## 7. UI 局部 keydown 保持原样（owner: 前端 teammate）

- [x] 7.1 验证以下组件**不动**任何 keydown 处理：`Modal.svelte`（Escape close + Tab focus trap）/ `Dropdown.svelte`（onAnchorKeydown）/ `CommandPalette.svelte`（内部方向键 / Enter / Escape，handleKeyDown）/ `SearchBar.svelte`（onKeydown）/ `ImageBlock.svelte`（lightbox Escape，svelte:window）/ `TabContextMenu`（document keydown for Escape）/ `SessionContextMenu`（document keydown for Escape）/ `UpdatePopover`（document keydown for Escape + outside click）/ `WorkspaceIndicator`（document keydown）/ `MemoryView`（document keydown）/ `Connection.svelte`（document keydown + 行内 Enter handler 保存 profile）—— 11/11 git diff main..HEAD 路径下未有改动，全部保留原始 listener
- [x] 7.2 在 `ui/src/lib/keyboard/registry.ts` 顶部加注释说明"局部 keydown 不并入 registry"的边界（详 design D6），并新增"多 instance shared shortcut 走同层 controller fanout"小节（D8 PaneContainer + session-detail-handlers 模式）

## 8. UI Settings 录键 widget（owner: 设计师 + 前端 teammate）

- [x] 8.1 设计师 teammate 在 Mailbox 投递 visual contract（idle / recording / conflict 三态视觉规约 + 录键 widget motion timing）
- [x] 8.2 新建 `ui/src/components/settings/KeyRecorderInput.svelte`：实现三态切换 / 焦点 trap / suspend-resume / commit-on-fullkey / Escape cancel；recording 期间组件自身调 `event.preventDefault()`（不依赖 dispatcher，因 dispatcher 已 suspend）
- [x] 8.3 新建 `ui/src/components/settings/ShortcutRow.svelte`：左 description / 中 KeyRecorderInput / 右"重置默认"按钮（spec category 之间分组样式由 KeyboardShortcutsPanel 控制）
- [x] 8.4 新建 `ui/src/components/settings/KeyboardShortcutsPanel.svelte`：listAll → groupBy(category) → 渲染 5 个 category section + 顶部"重置全部"按钮 + 顶部未保存提示条 + Save / 丢弃按钮 + 顶部"无法加载快捷键自定义" 错误条（IPC fallback 时显示）
- [x] 8.5 panel 内部维护 `pendingOverrides: Record<id, binding>` overlay；录键 commit 仅更新 overlay 不动 registry / cdt-config；Save 触发单次 `set_config` IPC + registry batch update + 清空 overlay
- [x] 8.6 panel 调 `findConflict(binding, sourceId, pendingOverrides)` 三参数 API；保存路径 SHALL 在 IPC 写入前对每个 pending entry 再走一遍 `findConflict` 防御串行注入
- [x] 8.7 `ui/src/routes/Settings.svelte`：在 `sectionList` 加 "键盘快捷键" 入口；handlers 路由到 `KeyboardShortcutsPanel`
- [x] 8.8 i18n：description 字段先用中文字面量（仓库未启用 i18n）

## 9. UI Settings 录键 widget 单测（owner: 前端 teammate）

- [x] 9.1 `KeyRecorderInput.test.ts`：进 recording 调 suspend / 退 recording 调 resume / commit 后 conflict 检测 / Escape cancel
- [x] 9.2 `ShortcutRow.test.ts`："重置默认"按钮在 currentBinding=defaultBinding 时 disabled
- [x] 9.3 `KeyboardShortcutsPanel.test.ts`：5 category 渲染 / 未保存提示条 / Save / 丢弃 / 重置全部
- [x] 9.4 录键时 dispatcher 不触发已注册快捷键（mock dispatch + 验证 sidebar.toggle 不被调用）

## 10. UI Playwright 用户故事（owner: QA teammate）

- [ ] 10.1 `ui/tests/e2e/keyboard-shortcuts.spec.ts`：14+ 条快捷键 happy path（每条 mockIPC + dispatch keydown + 验证效果）
- [ ] 10.2 录键交互 e2e：进 Settings → 改 sidebar.toggle 为 mod+shift+B → save → 验证旧 mod+B 不再触发、新组合触发
- [ ] 10.3 冲突检测 e2e：录入已占用 binding → 验证 conflict 反馈 + Save disabled
- [ ] 10.4 重置全部 e2e：自定义 3 条后点重置全部 → 验证 cdt-config 写入 empty + UI 显示 default
- [ ] 10.5 IPC error 回滚 e2e：mockIPC `setConfig` 返回 error → 验证 UI inline 错误 + registry 内存回滚

## 11. 性能 & 兼容（owner: 后端 + 前端 teammate）

- [x] 11.1 跑 `bash scripts/run-perf-bench.sh` 确认 perf-baseline 未回归（dispatcher 不在启动 / IPC / 列表渲染 hot path 上，预期无影响；如有意外 regress 按 `.claude/rules/perf.md::PR Perf impact 模板` 在 PR 描述贴四维数据）—— `perf_cold_scan` ✓ 无回归（wall=410ms Δ-18% / user=110ms Δ-26.7% / max_rss=32MB Δ-34.5% / user/real=0.268）；`perf_get_session_detail` SKIPPED（需 `-perf-fixture-project` corpus，本地无；keyboard dispatcher 不在 session 加载 hot path，跳过非阻塞）
- [x] 11.2 windows-compat-reviewer subagent 跑：确认 `event.code` 物理键兜底无 Windows 路径误判 —— PASS（0 P0 / 1 P1 / 4 P2 / 2 nit）；严格 scope 全部 clean：`platform.ts` non-mac 禁识 metaKey + AltGraph 守卫（`ctrlKey+altKey` 不误判 mod）+ `event.code` 物理键兜底（AZERTY / Dvorak / Numpad）+ home/encode/Path::is_absolute 等传统 trap 完全没踩。P1 跨平台 config drift（mac 录 `meta+x` 同步到 Windows 失效但 UI 仍展示 `Win+X`）+ 4 P2 录键 widget UX 边角 + 2 nit → `openspec/followups.md::keyboard-shortcuts::[windows-compat → followup]` 独立追踪
- [x] 11.3 macOS Cmd+W 已知系统冲突：在 KeyboardShortcutsPanel 该行 tooltip 加提示"在 macOS 与系统关闭窗口可能冲突，建议改键"

## 12. 文档与索引

- [x] 12.1 `openspec/followups.md` 新增 `keyboard-shortcuts` 段，记录 future: 引入 `tauri-plugin-global-shortcut` 让 `mod+shift+/` 可全局唤起 CommandPalette（应用未聚焦也响应）
- [x] 12.2 `ui/CLAUDE.md` 新增 "键盘快捷键" 段：指向 `ui/src/lib/keyboard/` registry，强调"全局 mod-key 走 registry / 局部 Escape/Enter 各组件自管"边界
- [x] 12.3 `crates/CLAUDE.md` 在"IPC 字段改动 checklist"段引用本次 `keyboardShortcuts: HashMap<String, String>` camelCase 字段

## 13. impeccable extract 提取 DESIGN.md delta（archive 前一刻）

- [ ] 13.1 跑 `/impeccable extract add-keyboard-shortcut-system` 提取候选 Named Rule + token + 组件规则
- [ ] 13.2 提进 `DESIGN.md`：
   - 新 Named Rule `The Conflict Is Warning Not Error Rule.`（Color §Named Rules 末尾）
   - 新 Named Rule `The Recorder Idle State Rule.`（Components §Inputs and search）
   - 新 token `--surface-recording-bg` / `--border-recording` / `--surface-conflict-bg` / `--border-conflict`
- [ ] 13.3 把 DESIGN.md 改动 commit 进同 PR

## N. 发布

- [ ] N.1 push 分支 + 开 PR（PR 描述含 Perf impact 模板 + 已跑 codex design 二审与 codex code 二审两次说明）
- [ ] N.2 wait-ci 全绿（按 `.claude/rules/opsx-apply-cadence.md` 与 codex 二审并行）
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

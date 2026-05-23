## Context

桌面应用基于 Tauri + WKWebView，浏览器层默认 `contextmenu` 事件未做兜底——当前应用中除了 Sidebar 会话项与 TabBar 标签项做了 `oncontextmenu` + `e.preventDefault()` 弹自定义菜单，其余所有位置的右键都漏到 WKWebView / macOS 系统菜单：

- **会话详情页消息正文** → 弹 macOS 文本菜单（Look Up / Translate / Search with Baidu / Speech / Services），并在右键 `mousedown` 阶段触发 WebKit smart-selection（光标下的"词"被自动选中）
- **空白区 / chrome 边缘 / 滚动条** → 弹 WKWebView 默认菜单（Reload / Inspect Element），用户误点 Reload 直接刷整个 app
- **Sidebar 会话项标题中的 worktree chip** → 截图证实右键时 `#claude-devtools-rs` 中的 "devtools" 被部分选中（`oncontextmenu` 里 `e.preventDefault()` 阻止的是菜单弹出，但 selection 已在 mousedown 阶段发生且未撤销）

PRODUCT.md 定位是"克制、可信、工程化"的桌面调试工作台——视觉接近 IDE/Linear；当前右键体验完全打破这一一体感。DESIGN.md 已有 `The Border Before Shadow Rule`（菜单是真浮层例外，可加 shadow）/ `The No Decorative Glass Rule`（无 blur）/ `The Status Owns the Color Rule`（菜单不引入装饰色），可直接复用现有 `SessionContextMenu` 视觉作为 `AppContextMenu` 起点——已 ship 验证两轮。

## Goals / Non-Goals

**Goals:**

- 全应用建立"右键 = app 自家菜单 / 不弹"的一致策略，永远不再漏到 WKWebView / macOS 系统菜单（除显式输入控件白名单）。
- 抽出 `AppContextMenu` items-driven 通用组件 + `use:contextMenu` Svelte action，作为后续 Phase 2 各 surface 加菜单的基础设施——一处定义视觉、定位、键盘可达性、smart-select 防护、生命周期。
- 修复用户截图中"右键 worktree chip 选中 'devtools'"的 smart-select bug。
- 键盘可达性：键盘 Menu 键 / Shift+F10 触发的 contextmenu 事件 SHALL 与鼠标右键走同一菜单；菜单内 ↑↓ Enter Esc 可完整操作。

**Non-Goals:**

- **Phase 2 各 surface 菜单清单不在本次实现**——仅记录在本文末 Future Scope 段，留下个会话开新 change 推进。
- **macOS 系统服务集成（Look Up / Translate / Speech / Share）不做**——用户已明确平时不用 macOS service，工作量大且收益靠后。
- **不修改** `sidebar-navigation` / `tab-management` spec 的右键菜单 Requirement——仅底层组件实现重构，菜单项、动作、触发位置维持当前契约。
- **不引入** 选中文本时的"app 风格文本菜单"（Copy / 在浏览器搜索）——属于 Phase 2 范围；Phase 1 内"选中 + 右键 = 不弹"是有意决定（比当前漏到系统菜单已是改善）。

## Decisions

### D1：全局兜底用 window-level `contextmenu` listener，capture 阶段 `preventDefault`，元素白名单按 closest 匹配

**选择**：在 `ui/src/main.ts` 启动时注册 `window.addEventListener('contextmenu', handler, { capture: true })`；handler 内：

```ts
function handler(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  if (!target) return;
  // 白名单：输入控件 + 显式 opt-in
  if (target.closest('input, textarea, [contenteditable="true"], [data-allow-native-context]')) return;
  // 已被自定义菜单元素的 use:contextMenu 处理（监听器先于 capture 阶段在元素上挂的 oncontextmenu 触发？否——两者都是 contextmenu 事件，按事件传播阶段决定先后）
  // → use:contextMenu 在元素上挂 oncontextmenu listener（target phase / bubble 阶段）；
  //    全局 capture handler 在 capture 阶段先到，但不能让它先 preventDefault 否则元素自己的 oncontextmenu 拿不到事件
  // → 解法：全局 handler 用 bubble 阶段（capture: false），且检测 e.defaultPrevented——若 use:contextMenu 已 preventDefault 并 stopPropagation 则不到这里；若未挂菜单则 e.defaultPrevented === false，由全局兜底 preventDefault
  if (e.defaultPrevented) return;
  e.preventDefault();
}
```

**最终方案**：全局 listener 走 **bubble 阶段**（capture=false），仅在 `e.defaultPrevented === false` 时 `preventDefault`。`use:contextMenu` 在元素本身挂 `oncontextmenu` listener，触发时调用 `e.preventDefault()` + 显示 menu，全局 handler bubble 到 window 时已是 prevented 状态，跳过。

**为什么不用 capture + 元素 dataset 标记**：因为 `data-allow-native-context` 类标记需要每个挂菜单的元素都打——增加心智负担；走 bubble + `e.defaultPrevented` 是天然的"已处理过"信号，无需额外协议。

**Alternative 1**：每个想要自定义菜单的元素都显式 `oncontextmenu={(e) => { e.preventDefault(); openMenu(e); }}`（当前做法）—— ❌ 没有兜底，仍漏菜单到非自定义元素。

**Alternative 2**：在 Tauri Rust 端 `tauri.conf.json` 禁用 webview 默认 contextmenu —— ❌ 一刀切太重，丧失 input/textarea 输入便利；且开发模式下需要 Reload/Inspect Element 调试。

### D2：`use:contextMenu` action 接受 items provider 而非 items 静态值，支持 lazy 计算

**选择**：

```ts
type ContextMenuProvider = ContextMenuItem[] | ((event: MouseEvent | KeyboardEvent) => ContextMenuItem[]);

function contextMenu(node: HTMLElement, provider: ContextMenuProvider) { ... }
```

`use:contextMenu={() => buildItemsForSession(session)}` 让 items 在右键当下计算（拿到最新的 session pinned/hidden 状态）；静态 `use:contextMenu={[...items]}` 仍可用。

**为什么**：当前 `SessionContextMenu` 的 `isPinned` / `isHidden` / `canSplit` 都是右键当下决定显示文案——若 `use:contextMenu` 只接静态 items，所有动态状态需走 `$state` 反应链，复杂度高；provider 函数是 React `useContextMenu` 等成熟模式的对齐做法。

**Alternative**：只接静态 items，调用方用 `$derived` 维护 → ❌ 调用方 boilerplate 重，violate "一处定义"目标。

### D3：菜单视觉沿用现有 `SessionContextMenu`，不引入 icon / 不增加分组 header

**选择**：`AppContextMenu` 视觉直接复用现 `SessionContextMenu` 的 token：

- bg `--color-surface` / border 1px `--color-border-emphasis` / radius 8px / padding 4px / shadow `0 4px 16px rgba(0,0,0,.15)`
- min-width 200px / item padding 7px 12px / item radius 4px / font-size 13px
- hover 用 `--tool-item-hover-bg`
- separator 用 `1px solid var(--color-border)`、margin `4px 8px`
- danger item（如"删除"）字色 `--color-danger`，hover 时 bg 染淡红——Phase 2 才会真正用上

**不加 icon**：DESIGN.md `The Tool Density Rule` 要求"产品 UI 不靠夸张装饰建立层级"+ PRODUCT.md "不为风格重造 affordance"。当前两个菜单纯文字已可读，加 icon 会让宽度 + 视觉权重发散。Linear / VS Code 部分菜单有 icon 但用法克制（用于区分 destructive vs. neutral）；Phase 2 引入"复制 / 在 Finder 显示 / 在终端打开"等 file-action 类动作时再考虑加 icon——届时单独走一轮 design 迭代。

**Alternative**：引入 lucide icon 一律左对齐 → ❌ 现有 5+ 项菜单已无 icon、加 icon 等于 visual breaking；后续 Phase 2 单独评估。

### D4：键盘 ↑↓ 导航实现走"focus 环 + manual `tabindex=-1`"模式，符合 WAI-ARIA APG menu pattern

**选择**（严格对齐 WAI-ARIA Authoring Practices Guide menu pattern）：

- 菜单容器 `role="menu"` `tabindex="-1"` `aria-orientation="vertical"`
- 每个 item `role="menuitem"` `tabindex="-1"`；disabled item 用 `aria-disabled="true"` 而非 `disabled` 属性，**保留键盘可达**（屏幕阅读器仍宣告"菜单项 X of N，已禁用"）
- 菜单打开时 SHALL 立即 active 第一个非 separator item（无论鼠标 / 键盘触发都一致）—— APG 标准行为，不区分触发源（早期版本 design 区分鼠标 vs 键盘是错误，违反 APG menu pattern；codex 二审报正）
- 方向键 ↑↓ 在所有 `role="menuitem"`（含 `aria-disabled` items）间循环移动 focus —— **不**跳过 disabled，否则屏幕阅读器用户无法感知该条目存在；分隔符（`role="separator"`）跳过
- Enter / Space 触发当前 focus item 的 action；若 item `aria-disabled="true"` 则 no-op（不调用 action 也不关菜单）
- 鼠标 hover 同步 `activeIndex` 到该 item，让键盘与鼠标焦点状态合一（避免两个独立 focus 模型）
- Esc 关闭，焦点还回 trigger 元素
- 键盘 `contextmenu` 事件（Menu 键 / Shift+F10）：触发到 element 时菜单定位到 element bbox 中心

**为什么不用原生 `<button>` 系列 + Tab key**：菜单是 transient 浮层，Tab 键应该让用户出菜单去 chrome 而非在菜单内移动；同时 menuitem 需要 `role="menuitem"` 而非 `role="button"` 让屏幕阅读器宣告"菜单项 1 of 7"。

**为什么 disabled 用 aria-disabled 而不是 disabled 属性**：原生 `disabled` 让元素从 a11y 树移除，键盘 ↑↓ 跳过该 item — 屏幕阅读器用户根本不知道存在"在新 Pane 打开（已达上限）"这一选项。`aria-disabled="true"` 保留可达性，让用户能感知"这个动作存在但当前不可用"——这才是 WAI-ARIA APG menu pattern 的标准做法。

### D5：smart-select 防护放在 `use:contextMenu` 内的 `mousedown` 监听，不靠 CSS `user-select: none`

**选择**：`use:contextMenu` 在元素上同时挂 `mousedown` 监听：

```ts
node.addEventListener('mousedown', (e) => {
  if (e.button !== 2) return;                          // 仅右键
  const sel = window.getSelection();
  if (sel && sel.toString().length > 0) return;        // 用户已 drag-select → 保留选区
  e.preventDefault();                                   // 阻止 WKWebView smart-select
});
```

**为什么不用 CSS `user-select: none`**：`user-select: none` 会让用户**永远无法在该元素内选中文本**——对会话标题这种简短 label OK，但 Phase 2 会给消息正文加 `use:contextMenu`，那时正文必须保留选中能力。`mousedown` 防护只在"无选区时阻止 smart-select"，**保留**用户先 drag-select 再右键的工作流（Phase 2 文本菜单将依赖此）。

**额外补丁**：`Sidebar.svelte` 的 `.session-item` CSS 同时加 `user-select: none`——双保险（session 标题不是用户用来选的内容；这是兜底以防 use:contextMenu 失败时仍不会触发 smart-select），并修截图同款 bug。

### D6：菜单关闭触发条件扩展为 outside-click + Esc + window blur + scroll + 视口变化

**选择**：

- outside-click：document `mousedown` 监听，`menuEl.contains(target)` 外即关
- Esc：document `keydown` 监听
- **新增** window `blur`：用户切到其它 app 时关菜单（macOS 切窗后再回来菜单仍浮着是反预期）
- **新增** 任意祖先元素 `scroll`：菜单随触发元素位置漂移会失锚——直接关
- **新增** window `resize`：viewport 变化菜单位置会越界——直接关

**为什么 scroll 不做 reposition**：reposition 需要追踪 trigger 元素 bbox，复杂度高；用户右键习惯里菜单出现 → 立刻交互（< 2s），不会先滚屏再回去点菜单——直接关更符合预期。

### D7：浮层 SHALL portal 到 `document.body`，不在 trigger 元素内部 inline 渲染

**选择**：`AppContextMenu` 在 `use:contextMenu` 触发时通过 Svelte 5 `mount()` API（或等效 portal pattern）渲染到 `document.body` 末尾，**不**作为 trigger 元素的子元素 inline 渲染。

**为什么**：

1. **避免 overflow clipping**：sidebar 会话项 `.session-item` 处在 `overflow-y: auto` 虚拟滚动容器内、tab item 在 `.tab-list` `overflow-x: auto` 内——若菜单 inline 渲染会被父容器 clip。本仓 `Sidebar.svelte::1109` 的 `<SessionContextMenu>` 当前已在 sidebar `<aside>` 外渲染（同级 `.app` 末尾）正是出于此考虑，是已验证先例。
2. **z-index stacking 简化**：portal 到 body 末尾天然处于最高 stacking context，无需在调用方层层覆写 z-index，也不会被祖先的 `transform` / `filter` / `contain` 创建新 stacking context 隔离。
3. **`menuEl.contains(target)` 外点判断正确性**：portal 后菜单 DOM 在 body 末尾，trigger 元素的 mousedown 事件**不**会被认为是"菜单内"——这正是外点关闭逻辑期望的（点 trigger 关菜单 + 重新触发新菜单）。inline 渲染则需额外排除 trigger 元素自身。
4. **focus return 路径稳定**：trigger 元素引用在 `use:contextMenu` action 闭包内持有，菜单关闭时 `triggerNode.focus()` 稳定可达，不依赖 DOM 树位置。

**Alternative 1**（在 trigger 元素相邻渲染 `position: absolute` 浮层）：❌ overflow clip 风险 + z-index 复杂度高 + Svelte 5 reactivity 在 trigger 跨 portal 边界时仍需手动管理。

**Alternative 2**（在 Tauri 原生窗口层用 `WebviewWindow` 渲染菜单）：❌ 跨进程通信延迟、与 vitest / Playwright 测试不兼容、无法与 jsdom 单测；不必要的复杂度。

**实现细节**：`use:contextMenu` 持有内部 `let menuInstance: ReturnType<typeof mount> | null` 引用；右键时若已有 instance 先 unmount 再创建新的；action destroy 钩子 unmount 兜底。Portal 不引入新依赖（Svelte 5 原生支持）。

### D-V1：菜单作为真浮层例外允许 shadow，但严格遵守 `The No Decorative Glass Rule`（无 backdrop-filter）

DESIGN.md `The Border Before Shadow Rule` 要求"先用 surface + border + hover 解决层级，只有浮层或明确 hover lift 才加 shadow"——右键菜单是真浮层（脱离 flow，悬浮在内容上方），符合 shadow 例外。但**不**加 `backdrop-filter: blur()` —— DESIGN.md `The No Decorative Glass Rule` 明令禁止；shadow + 实底 surface 已足够从 SessionDetail / Sidebar 内容分离。

shadow 值 `0 4px 16px rgba(0,0,0,.15)` 沿用现 `SessionContextMenu`，对齐 DESIGN.md Section 4 floating overlay 形态。

### D-V2：菜单**不**作为 selection 信号——hover/active item 仅用 `--tool-item-hover-bg`，不沾 Focus Blue

DESIGN.md `The Persistent Selection Is Quiet Rule` 把 Focus Blue 留给瞬时焦点 + ongoing/live。菜单 item 的 hover/keyboard active 是**瞬时焦点**——按理可以用 Focus Blue。但本次决策仍走暖中性 hover bg：

- **理由**：菜单整体是 transient 短暂浮层（< 2s 用户决策窗），用 Focus Blue 反而引入"持久选中"误解；hover bg 已足够提供选中反馈
- **可选 focus ring**：键盘 nav 时 active item 加 `outline: 2px solid rgba(59,130,246,.15)` 作为瞬时键盘焦点提示——这是合规模式（瞬时 + a11y 必需）

## Visual Contract

### Surface Decision

**入口选择**：右键菜单作为新 surface，挂在所有非输入控件的元素上（whitelist 见 D1）。

- 替代旧 surface：WKWebView / macOS 系统菜单 ❌（违反 PRODUCT.md `## Anti-references` "不要做成营销页、聊天玩具或霓虹终端"——系统菜单含"Search with Baidu / Speech"等无 app 关联性的 action，破坏"克制、可信、工程化"调性）
- 与已有 surface 关系：等价替换 `SessionContextMenu` / `TabContextMenu`；为后续 Phase 2 新菜单（chunk / 工具结果 / chip）提供唯一入口

### Visual Layer

- 形态：`AppContextMenu` 浮层 — 单 column 文字菜单 + separator + (Phase 2) danger 类
- bg / border / radius / shadow：沿用 `SessionContextMenu` token（详 D3）；引用 `DESIGN.md::The Border Before Shadow Rule`（浮层例外）+ `DESIGN.md::The No Decorative Glass Rule`（无 blur）
- item hover：`--tool-item-hover-bg` —— 引用 `DESIGN.md::The Persistent Selection Is Quiet Rule` 的"瞬时焦点不强制 Focus Blue"边界（详 D-V2）
- 排版：`13px / 1.4 / 400`，与现 `.cm-item` 一致；属 DESIGN.md `## 3. Typography::Body` 范畴
- 不引入新色（不染 success-green / failure-red / amber / blue/indigo 等品牌色作装饰）—— 引用 `DESIGN.md::The Status Owns the Color Rule`

### State Coverage

| 状态 | 触发 | 视觉 / 行为 | 实现位置 |
|---|---|---|---|
| **default** | 菜单刚弹出 | 第一项 keyboard active（仅鼠标触发时不预 active；键盘触发时预 active）| `AppContextMenu.svelte::onMount` |
| **hover** | 鼠标移动到 item | bg `--tool-item-hover-bg` | CSS `.cm-item:hover` |
| **keyboard active** | ↑↓ 移动到 item | bg `--tool-item-hover-bg` + outline `2px rgba(59,130,246,.15)` | `AppContextMenu.svelte::handleKeyDown` |
| **disabled** | item.disabled === true | opacity 0.45 / cursor not-allowed / 键盘 ↑↓ 跳过 | CSS `.cm-item-disabled` |
| **danger** | item.danger === true | 文字色 `--color-danger`、hover bg 染淡红 | CSS `.cm-item-danger` |
| **action feedback** | item action 后短暂"已复制!" 提示 | label 切换为 feedback 文案 600ms 后关闭 | `AppContextMenu.svelte::triggerAction` |
| **closed: outside click** | document mousedown 在 menu 外 | 直接 unmount | `use:contextMenu::outsideHandler` |
| **closed: Esc** | document keydown Esc | 直接 unmount + 还焦到 trigger | 同上 |
| **closed: window blur** | window blur | 直接 unmount | 同上（**新增**）|
| **closed: scroll** | 任意祖先 scroll | 直接 unmount | 同上（**新增**）|
| **closed: resize** | window resize | 直接 unmount | 同上（**新增**）|
| **edge clamp** | 触发位置贴近 viewport 边 | x / y 距边 ≥ 8px | `AppContextMenu.svelte::clampedX/Y $derived` |
| **smart-select 防护** | 右键 mousedown 无选区 | preventDefault 阻止 WKWebView 选词 | `use:contextMenu::mousedownHandler` |
| **selection preserved** | 右键 mousedown 已有选区 | 不动选区（让 Phase 2 文本菜单可消费）| 同上 |
| **键盘触发** | Menu 键 / Shift+F10 在 trigger 上 | 菜单定位到 trigger bbox 中心 | `use:contextMenu::keydownHandler` |
| **focus return** | 菜单关闭 | 焦点还到 trigger 元素 | `use:contextMenu::onClose` |

### DESIGN.md delta plan

本 PR 引入"`AppContextMenu` 通用浮层"作为可复用 token——archive 前 SHALL 跑 `/impeccable extract` 把以下沉淀为 DESIGN.md `## 5. Components` 新子节：

- 新增 `### Context menu` 子节，列：bg / border / radius / shadow / item padding / hover bg / disabled / danger / 触发模式（mouse + keyboard）
- 新增 Named Rule **"The App Owns the Right-Click Rule."**：右键事件全应用兜底 preventDefault；输入控件 + `[data-allow-native-context]` 例外。任何新增 contextmenu 处理 SHALL 走 `use:contextMenu` action，禁止裸 `oncontextmenu` 单点实现

## Risks / Trade-offs

- **[Risk] 全局兜底误伤未来需要系统菜单的元素** → Mitigation: `data-allow-native-context` 显式 opt-in 出口写进 spec；在 design.md 这条决策上留 docstring，新加该属性时自然查到
- **[Risk] Tauri 开发模式下用户失去 Reload / Inspect 右键** → Mitigation: 开发模式默认开 devtools 窗口；Cmd+Shift+I / Cmd+R 等键盘快捷键不受影响；prod 模式本就不该有这些菜单
- **[Risk] 重构 `SessionContextMenu` / `TabContextMenu` 改坏现有行为** → Mitigation: vitest 保留现有 SessionContextMenu / TabContextMenu 单测、新增基于 AppContextMenu 的回归测；Playwright sidebar / tab 右键 e2e 不应改（外部 API 兼容）
- **[Risk] `mousedown` 阻止 smart-select 影响 macOS Service / Look Up 等系统服务** → Mitigation: 这正是设计意图——Phase 1 不要系统菜单；Phase 2 重新评估是否在文本菜单内重新接入
- **[Risk] 菜单 keyboard ↑↓ 在大量 Svelte 5 reactivity 下性能** → Mitigation: items 列表通常 ≤ 10 项，`activeIndex` 是单 `$state` number，无性能问题
- **[Trade-off] Phase 1 内"选中文字 + 右键 = 不弹"会让习惯了 macOS 文本菜单 Copy 的用户找不到入口** → Mitigation: Cmd+C 仍可用；Phase 2 即将提供 app 文本菜单；Phase 1 PR 描述里写明这一过渡

## Migration Plan

1. **新增基础设施文件**（不影响现有功能）：
   - `ui/src/lib/components/AppContextMenu.svelte`
   - `ui/src/lib/contextMenu.svelte.ts`（含 `contextMenu` action + `installGlobalContextMenuFallback()`）
2. **`ui/src/main.ts`** 启动时调 `installGlobalContextMenuFallback()`——立刻消除"漏到 OS 菜单"问题
3. **重构 `SessionContextMenu.svelte`** 改用 `AppContextMenu`，外部 props 兼容
4. **重构 `TabContextMenu.svelte`** 同上
5. **CSS 补丁**：`Sidebar.svelte::.session-item` 加 `user-select: none`（双保险，独立修截图同款 bug）
6. **测试**：vitest 单测三态决策 / smart-select 防护；Playwright e2e 键盘 ↑↓ Enter Esc + Menu 键
7. **验收**：跑 `cargo tauri dev` 真桌面手测——任意位置右键不再弹 macOS 菜单；sidebar / tab 右键回归正常

回滚策略：基础设施文件 + main.ts 调用一行删除 = 完整回滚；`SessionContextMenu` / `TabContextMenu` git revert 即可恢复。无 IPC / 数据迁移，零风险。

## Future Scope（Phase 2 — 后续会话开新 change）

**目标**：在 Phase 1 基础设施上，给以下 surface 加 app 上下文菜单 items；同时引入"app 风格文本菜单"。

### 各 surface 菜单清单（候选）

| Surface | 菜单项（候选）|
|---|---|
| User chunk（用户消息）| 复制整条 prompt / 复制为 markdown / 跳到这条之前的上下文 |
| AI chunk（AI 消息）| 复制整条回复 / 复制为 markdown / 折叠这条 / 跳到这条 |
| Bash 工具块 | 复制命令 / 在终端运行 / 复制 stdout / 复制 stderr |
| Read / Edit / Write 工具块 | 复制文件路径 / 在 Finder 显示 / 在编辑器打开（按 Settings 选 VS Code/Cursor）|
| Worktree chip / 项目卡 | 复制路径 / 在 Finder 显示 / 在终端打开 / 隐藏 |
| 选中文本（任意区域）| Copy / Copy as Plain Text / 在浏览器搜索（默认 Google，可 Settings 改）|

### Phase 2 spec delta 范围（届时 propose 时确认）

- `session-display` 加"消息右键菜单" Requirement
- `tool-execution-linking` 加"工具结果右键菜单" Requirement
- `sidebar-navigation` 加"worktree chip / 项目卡右键菜单" Requirement
- `frontend-context-menu` 加"文本菜单" Requirement（依赖 `mousedown` 已选区时不阻止的 Phase 1 决策）

### Phase 2 设计开放问题

- 文本菜单"在浏览器搜索"是否允许 Settings 配置默认搜索引擎？
- 文件 action（在 Finder 显示 / 在编辑器打开）走 Tauri opener plugin 还是新增专用 IPC？
- icon 是否在文件 action 类菜单引入（lucide），还是保持纯文字？

这些待 Phase 2 propose 时再决定，不阻塞 Phase 1。

## Open Questions

无。所有设计决策已在 D1-D6 / D-V1-D-V2 收敛。

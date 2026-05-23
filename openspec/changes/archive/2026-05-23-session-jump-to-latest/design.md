# Design

## D-V1：浮层位置选 conversation 容器内右下角，不放 ContextPanel toolbar / 顶部 toolbar / 底部 strip

**候选方案**：

- **A. conversation 容器内 `position: absolute; bottom right` 浮动**（TS 原版做法）
- **B. ContextPanel toolbar 内塞按钮**
- **C. SessionDetail 顶部 toolbar（与 SearchBar / metrics chip 同条）**
- **D. 底部独立 strip / footer**

**决策**：选 **A**。理由：

- **B 不行**：ContextPanel 默认折叠，发现不到；且 ContextPanel 现有锚点都按 phase / category 语义（"跳到 phase 1 起点"），与"跳到最新"动作不同语义通道，混合会破坏 panel 纯粹性
- **C 不行**：顶部已是 `UnifiedTitleBar` + `TabBar` 两层 chrome，再叠按钮违背 `Tool Density Rule`；且"回最新消息"是**内容区导航**，不是 chrome 操作，放顶部语义错位
- **D 不行**：占持久空间，违背"按需 affordance"原则——按钮 95% 时间无效（用户大多贴底自动跟随）；且会与已嵌在 chunk 流末尾的 `OngoingBanner` 在垂直方向竞争视觉权重，违反 `One Live Signal Rule` 精神
- **A 正确**：affordance 在动作发生处出现（Fitts's Law）；ContextPanel 让位逻辑沿用原版；OngoingBanner 是 inline 不是 absolute，与按钮在不同 stacking layer 不互覆盖

链回 PRODUCT.md `Anti-references`（不做装饰）+ `Design Principle 4`（状态优先于装饰）+ DESIGN.md `The Persistent Selection Is Quiet Rule` 互补面（持久 UI 安静；瞬时 affordance 仅在动作适用时浮起）。

## D-V2：不做"新内容到达"dot 徽章

**候选方案**：

- **A. 加 dot 徽章**（颜色待定）
- **B. 不加任何徽章，按钮存在 = "可以回最新"已是足够 affordance**

**决策**：选 **B**。理由：

- 用户既然主动上滚就不在主线追新（在阅读历史 / 验证某段输出），新内容到达不一定要立刻看
- 加 dot 必须开**新色彩通道**——蓝（被 ongoing/live 占）/ 绿（被 success / completed 占）/ 琥珀（compaction / warning）/ 红（failure）/ 紫（thinking）全部分派完，引入第 N 种"未读"色违反 `The Status Owns the Color Rule`
- 改用 motion pulse 表达"有新内容"：仍引入新动效模式，且与 reduced-motion 用户无效
- 极简先 ship；未来真有用户反馈"想知道有新消息到达"再加，到时一并解决色彩通道开哪种语义的问题

## D-V3：键盘绑定走最小集，不做回顶 / 不做 End/Home 兜底

**候选方案**：

- **A. 全集六绑定**：mac `⌘+↓`/`⌘+↑` + Win/Linux `Ctrl+End`/`Ctrl+Home` + 跨平台兜底 `End`/`Home`
- **B. 中等四绑定**：mac `⌘+↓`/`⌘+↑` + Win/Linux `Ctrl+End`/`Ctrl+Home`，无兜底
- **C. 最小二绑定**：mac `⌘+↓` + Win/Linux `Ctrl+End`，仅去底无回顶

**决策**：选 **C**。理由：

- 核心需求是"回最新"，鼠标按钮也只去底（`chevrons-down` icon），键盘与之语义对称
- 回顶是次要诉求，多数长会话场景里用户已通过 ContextPanel phase 视图或 `Ctrl+Home`-原生（input 内）导航顶部
- `End`/`Home` 兜底键在 macOS 笔记本上无原生键，跨平台一致性不足；多一组绑定增加学习成本而无对称收益
- 回顶 + Home 兜底纳入 Future Considerations，按用户反馈再补

**触发条件**（两条 guard 同时满足才拦截）：
1. **input 排除**：`document.activeElement` 不是 `input` / `textarea` / `contenteditable`。否则走原生光标导航（搜索框里 `⌘+↓` 移光标到末尾、`Ctrl+End` 同义），尊重浏览器默认行为
2. **focused pane 排除**（codex P1#2 反馈）：`getActiveTabId() === tabId`。SessionDetail 的 `handleKeydown` 当前挂在 `document` 上（line 134 `document.addEventListener("keydown", ...)`），PaneView 多 pane 场景下每个 SessionDetail 都各自挂一份 listener；如果不 guard 谁拦谁，按一次快捷键所有 mount 中的 SessionDetail 都会滚到底（pane A 上滚阅读历史时按 `⌘+↓` 期望 A 跳底，pane B 也会跟着跳）。`tabStore.svelte.ts::getActiveTabId()` 返回 `focusedPane().activeTabId`——只有当前 focused pane 内 active 的 SessionDetail 满足 `getActiveTabId() === tabId`，其它 pane 的 SessionDetail 直接 return 不拦截

**Tooltip 平台分流**：通过 `lib/platform.ts::isMac()` 切换；mac 显示 `跳到最新消息 (⌘↓)`，Win/Linux 显示 `跳到最新消息 (Ctrl+End)`。

## D-V4：programmatic-scroll 状态机用 scrollend + 用户输入打断 + 距底兜底，不靠 timer

**问题**（codex P1#1 反馈）：早期 brief 写"程序滚动期间设 isProgrammaticScroll，300ms 后清"——这是 buggy 设计：

- smooth scroll 实际时长由浏览器、距离、低端机渲染速度决定。长 session 滚 5000px 在 Tauri WKWebView 上可能 600-800ms，固定 300ms timeout 提前清 → 期间 scroll 事件触发 `isFar` 重新派生 → 按钮在滚动半路重新显现
- 用户连续点按钮 N 次（在 smooth scroll 期间再点），固定 timeout 不持 ID 时旧 timeout 会提前清掉新 scroll 状态
- 用户在 smooth scroll 期间用手势（wheel / touch / 任意 keydown）打断滚动，state flag 仍在挂着 → 按钮被错误抑制

**方案**：

```svelte
let isProgrammaticScroll = $state(false);
let progScrollTimer: number | null = null;

function startProgrammaticScroll() {
  isProgrammaticScroll = true;
  if (progScrollTimer !== null) clearTimeout(progScrollTimer);
  progScrollTimer = window.setTimeout(stopProgrammaticScroll, 1500);  // fallback
}

function stopProgrammaticScroll() {
  isProgrammaticScroll = false;
  if (progScrollTimer !== null) {
    clearTimeout(progScrollTimer);
    progScrollTimer = null;
  }
}

// 1) scrollend 事件主条件（Tauri WKWebView / WebView2 已支持）
conversationEl.addEventListener("scrollend", stopProgrammaticScroll, { once: false });

// 2) 距底 ≤ 16px 兜底（与 wasAtBottom 同阈值；处理 scrollend 不触发的边缘情况）
//    放在 scroll listener 内：if (isProgrammaticScroll && nearBottom16) stopProgrammaticScroll();

// 3) 用户主动输入立即打断
conversationEl.addEventListener("wheel", maybeCancelProgScroll, { passive: true });
conversationEl.addEventListener("touchmove", maybeCancelProgScroll, { passive: true });
// keydown 单独处理：handleKeydown 内仅"非我们触发的快捷键"才打断
//   （我们触发的 ⌘+↓ / Ctrl+End 走 startProgrammaticScroll → 自洽，不能立即打断自己）
```

**onDestroy / {@attach} cleanup**（codex P2#4 关联）：`removeEventListener` 三类 + `clearTimeout(progScrollTimer)` + `cancelAnimationFrame(rAF id)`。

**为什么不用 1500ms 单独兜底就够**：scrollend 事件在 Safari 17+ / Chrome 114+ / WebView2 已稳，是更精确的真完成信号；timer 仅作 fallback 防 scrollend 在某些 emulated 环境（如 reduced-motion 下 `behavior: 'auto'` 是否触发 scrollend）失踪。

## D-V5：阈值 300px 沿用 TS 原版常量；与既有 16px wasAtBottom 阈值不冲突

两个阈值用途不同：

- **16px**（`refreshDetail::wasAtBottom`）：自动跟随判定——新内容到达时是否自动滚到底
- **300px**（本 change 引入 `JUMP_THRESHOLD`）：按钮显隐判定——用户当前距底是否远到需要 affordance

300px 等于约 4-5 行 chunk 高度。<300px 时用户视线仍能扫到底部内容，affordance 价值低；>300px 时已脱离底部 viewport 范围，affordance 价值高。

沿用原版数值降低跨实现差异；后续如发现噪音过大可调（比如 500px）但不在本 change scope。

## D-V6：组件抽不抽——以代码量决定

按钮逻辑预估 ~80-120 行（含 hover/focus/pressed/scrolling 状态机 + scroll 监听 + keyboard 监听 + reduced-motion 降级）。

**决策**：先 inline 进 `SessionDetail.svelte`（已 2177 行，多 100 行不会撑爆）；apply 阶段实现完后回看，**如果**抽出来对 `SessionDetail.svelte` 主体可读性有正贡献且组件足够独立（不依赖 SessionDetail 内部状态外的 prop）则抽 `JumpToLatestButton.svelte`。

不预先承诺抽组件——避免"为复用而抽"陷阱，目前没有第二处使用场景。Future Considerations 里的"Sidebar 长列表回顶"如果落地，那时再抽 `JumpToAnchorButton.svelte` 通用组件。

---

## Visual Contract

### Surface Decision

- 入口：`SessionDetail.svelte` 内 `.conversation` scroll 容器内 `position: absolute; bottom: 16px; right: 16px; z-index: 10`
- **ContextPanel 让位**（codex P2#3 反馈修正）：`ContextPanel.svelte:298` 实际是 `width: min(320px, 100%)` 没有共享 JS 常量，原 brief 写"复用 SessionDetail 既有常量 `CONTEXT_PANEL_WIDTH`"是错的。正确做法走 **CSS-only 让位**，避免运行时 measure：
  - SessionDetail 在打开 ContextPanel 时给 conversation 容器加 class `.has-context-panel`
  - 按钮 CSS：`right: 16px; .has-context-panel & { right: calc(min(320px, 50vw) + 16px); }`（用同一个 `min(320px, 50vw)` 表达式与 ContextPanel CSS 对齐——这里用 `50vw` 而非 `100%` 因为 SessionDetail 是 main pane 的子元素，避免按钮在窄屏 < 320px 时被推出可见区）
  - 这样：① 不需要 JS 常量；② ContextPanel 宽度未来调整时改一处；③ 窄屏行为更可控
  - 实现细节抽到 `app.css` `--session-jump-button-right` CSS var：默认 `16px`，`.has-context-panel` 下覆盖为 `calc(min(320px, 50vw) + 16px)`，按钮直接读 `right: var(--session-jump-button-right)`——同源 token 化让 ContextPanel 实际宽度调整时按钮自动跟随
- Stacking：按钮 `z-index: 10` 在 conversation 内部；OngoingBanner inline 在 chunk 流末尾（不是 absolute），两者不同 stacking layer 不冲突
- 链回 PRODUCT.md `Anti-references` + `Design Principle 4` + DESIGN.md `The Persistent Selection Is Quiet Rule` 互补面

### Visual Layer

- **形态**：icon-only `chevrons-down`（lucide path，从 `lib/icons.ts` 导出 `iconChevronsDown`），无视觉文字
- **尺寸**：28×28 hit area / 14px 图标 / `6px` radius —— 引用 `DESIGN.md::Buttons and icon controls`（28-36px hit area / 13-16px icon 中位）
- **颜色**：bg `--color-surface-raised` + 1px `--color-border-emphasis` + 图标 `--color-text-secondary`。**禁用** Focus Blue / indigo / 任何语义色 —— 引用 `The Status Owns the Color Rule` + `The Ongoing Owns Blue Rule`（这是 affordance 不是 live signal，不参与状态色彩通道）
- **Elevation**：`0 2px 8px rgba(0,0,0,0.06)` 默认；hover 升至 `0 4px 12px rgba(0,0,0,0.08)` —— 引用 `The Border Before Shadow Rule` 的"浮层例外"，与 Dashboard card hover lift 同档（不是装饰大阴影）
- **进出动效**：`opacity + translateY 8px`，进 200ms `cubic-bezier(0.16, 1, 0.3, 1)`（ease-out-quart，与 anchor-pulse 同曲线）/ 出 150ms ease-out；不动 height/width/margin（避免 layout 重排，引用 DESIGN.md Section 5 Motion 末条）
- **`prefers-reduced-motion: reduce`** 降级：进出 = 即时显隐 + scroll `behavior: 'auto'`

### State Coverage

| 态 | 视觉 | 实现位置 |
|---|---|---|
| **default**（距底 ≤ 300px） | 隐藏（`opacity: 0; pointer-events: none; translateY(8px)`） | `SessionDetail.svelte` derived `isFar` |
| **visible**（距底 > 300px） | bg surface-raised + emphasis border + 14px chevrons-down + secondary text | inline `<button>` 或 `JumpToLatestButton.svelte` |
| **hover** | bg → `--color-surface-overlay`（再抬一档） + shadow 升档 + cursor pointer + 0.12s ease-out | `:hover` |
| **focus-visible** | visible 态 + outline `0 0 0 2px rgba(59,130,246,.15)` | `:focus-visible`（仅键盘 Tab 触发，鼠标 click 不出现） |
| **pressed** | `transform: scale(0.96)` + bg 同 hover + 200ms ease-out 回弹 | `:active` |
| **programmatic-scrolling-suppressed**（点击 / 键盘触发的 smooth scroll 期间） | 立即隐藏，期间不重新触发显示判定 | `isProgrammaticScroll` flag，300ms 后清 |
| **dark theme** | bg `--color-surface-raised`(dark `#2a2a27`) + border `--color-border-emphasis`(dark `#4f4e4a`) + text-secondary(dark `#a8a5a0`) | `app.css` 既有 token 双套 |
| **reduced-motion** | 进出即时 + scroll `behavior: 'auto'` | `@media (prefers-reduced-motion: reduce)` |
| **tab 切走 / 切回** | 不持久化按钮显隐状态；切回时根据当前 `scrollTop / scrollHeight` 重新判定 | `uiState.scrollTop` 已存，按钮可见性是 derived 不需独立存 |

### DESIGN.md delta plan（archive 前 SHALL 落地）

新增 `## 5. Components` 末尾子节 **`### Floating affordances`**：

```markdown
### Floating affordances

按需浮现的内容区导航按钮（如 SessionDetail「跳到最新消息」）。

- **位置**：内容滚动容器内 `position: absolute`，距右下角各 `16px`；右侧浮层（如 ContextPanel）打开时 `right += panelWidth`。
- **形态**：icon-only，28×28 hit area，14px 图标，`6px` radius。
- **颜色**：`--color-surface-raised` bg + `1px solid --color-border-emphasis` + `--color-text-secondary` 图标色。**不允许**使用 Focus Blue / indigo / 任何语义色——这是 affordance 不是状态。
- **Elevation**：`0 2px 8px rgba(0,0,0,0.06)`（与 Dashboard card hover 同档）；hover 时升至 `0 4px 12px rgba(0,0,0,0.08)`。引用 `Border Before Shadow Rule` 的"浮层例外"条款。
- **进出**：`opacity + translateY 8px`，进 200ms `cubic-bezier(0.16, 1, 0.3, 1)`（ease-out-quart）/ 出 150ms ease-out；`prefers-reduced-motion: reduce` 下退化为即时显隐。
- **States**：default / visible / hover / focus-visible / pressed / programmatic-scrolling-suppressed 六态完备。
- **A11y**：必备 `aria-label`，键盘 Tab 可达，focus-visible 用 `0 0 0 2px rgba(59,130,246,.15)` outline。
```

新增 Named Rule（放在 `## 5. Components` 末尾，与 Section 5 现有 ad-hoc 段落同级；它跨多个未来组件）：

```markdown
**The Floating Is Affordance, Not Decoration Rule.** 浮层按钮仅在**动作语义**存在时显现（如长滚动列表的"回最新消息"、长 prose 的"回顶"）。一旦动作不再适用即隐去，**不**作为持久导航或装饰存在。这条规则与 `The Persistent Selection Is Quiet Rule` 互补：持久选中默认安静；瞬时 affordance 则可以浮起，但**仅在它能完成的那个动作仍然有意义时**。
```

**不引入新 token / 颜色变量** —— surface-raised / border-emphasis / surface-overlay / text-secondary 全部已存在。

---

## Future Considerations

不在本 change scope，按用户反馈再开：

- **回顶**：⌘+↑ / Ctrl+Home + chevrons-up 按钮，与"回最新"对偶。前提：用户报告"长会话回顶也痛"
- **End / Home 兜底键**：跨平台一致性补全；前提：用户报告 macOS 外接键盘 End 键期望
- **"新内容到达"dot 徽章**：先解决"开哪种语义色"的色彩通道问题（候选：新增 attention amber 但与 compaction warning 区隔；候选：用 motion pulse 不开色彩通道）
- **`Cmd+Enter` 跳到末尾 user 输入**：power user 路径，待会话超长场景验证 ROI
- **沉淀通用 `JumpToAnchorButton.svelte` 组件**：前提是出现第二处使用场景（如 Sidebar 长列表回顶 / Markdown 长文回顶）
- **多锚点导航（上一/下一条 user 消息）**：由 ContextPanel phase 视图承担；如未来 phase 视图被简化或移除，本 change 再扩

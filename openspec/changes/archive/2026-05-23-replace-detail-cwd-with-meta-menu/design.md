## Context

`SessionDetail.svelte` 顶 bar 当前布局：

```
.top-bar (row flex)
├── .top-titles (column flex, flex: 1, min-width: 0)
│   ├── h1 (font-size 18, ellipsis nowrap)
│   └── .top-stats (row flex, flex-wrap: wrap, font-mono 11px)
│       └── [..., LAST 19:50:46 · CWD <full-path>]
└── .top-meta (flex-shrink: 0)
    └── [Context N] (.top-badge)
```

`top-stats` 末位的 CWD 长串触发 `flex-wrap` → 整段被推到第二行，第一行末尾留下孤悬 `·`，主从视觉权重颠倒（mono 长串重于 h1）。详见 `proposal.md::Why`。

**已有约束**：
- `tauri-plugin-opener` 已注册（用于 `OpenInBrowser` 等场景），`open_path` 命令可直接 spawn 系统文件管理器。
- `.top-meta` 区已是 `flex-shrink: 0` 右上 toolbar，`.top-badge` 是既有控件 token（padding 6px 10px / radius 6px / 13px icon），与 `[Context]` 同区扩 button 不破结构。
- HTTP server mode (`?http=1`) 无 Tauri runtime，不能调 plugin；本 change SHALL 在该模式下隐藏 Finder 项。
- 原版 TS `claude-devtools/src/renderer/` 不渲染 cwd——本 change 偏离原版。

**stakeholder**：会话详情页用户（高频阅读 + 低频跳出操作），无后端 / cdt-api 依赖方。

## Goals / Non-Goals

**Goals**：
1. 顶 bar 视觉常态消除 CWD 噪音，h1 主题恢复主导视觉权重。
2. 保留低频但真实的 cwd 操作能力（在文件管理器打开 / 复制路径），通过 on-demand menu 承载。
3. 顺手把 menu 设计为「session 级 meta-action」语义入口，未来扩 session 操作（导出 / 复制 ID / 复制 deeplink 等）不破结构。
4. 与既有 `.top-meta` toolbar + `.top-badge` token 复用对齐，不重造控件。

**Non-Goals**：
1. **不**做「在 Terminal 中打开」一键能力——跨 OS（macOS Terminal/iTerm/Warp/Wezterm/Ghostty / Linux gnome-terminal/konsole/alacritty / Windows wt/cmd/PowerShell）+ 用户偏好 settings 是独立子题，单独 change 评估。
2. **不**改 cwd 数据通路（`detail.metadata.cwd` 仍由 `cdt-api::get_session_detail` IPC 提供，菜单消费同源数据）；不动 IPC 字段。
3. **不**承载不属于"会话工作目录 / 标识"语义的操作（如修改 session 标题、删除 session 等）—— 删除 / 重命名归 `SessionContextMenu`（侧栏右键菜单）。
4. **不**新增 "会话 metadata 浮卡片"（之前 explore 阶段一度考虑的 ⓘ popover 形态被否决，理由见 D-V1）。
5. **不**把 cwd 移到 `ContextPanel` —— ContextPanel 是「注入到 Claude 的上下文」语义（claude-md / mentioned files / tool outputs / thinking text / task coordination / user messages），cwd 不是注入物。

## Decisions

### D1. 菜单组件：独立 `SessionMetaMenu.svelte` 组件，内部用 `Dropdown.svelte` portal/overlay 行为

**选**：独立组件包装 menu trigger + items；overlay 行为复用 `lib/components/Dropdown.svelte` 已实现的 outside-click / Esc 关闭 / 键盘导航 / portal 锚定。
**否**：直接在 `SessionDetail.svelte` 内 inline 一段 `<button>` + `<div role="menu">` —— 重复造 outside-click / focus trap / a11y 行为，且与 `Dropdown.svelte` 的 token 走两条路。
**理由**：`ui/CLAUDE.md::UI 组件规范` 明确 SHALL 用 `lib/components/Dropdown.svelte`，禁止原生 `<select>` 或自造下拉。menu 比 select 多一层（item 是 action 不是 value），但 overlay 几何一致；`Dropdown` 现 API 若不支持 action menu 模式，apply 阶段评估「扩 Dropdown.svelte 加 menu mode」 vs 「新建 sibling `Menu.svelte` 共享 portal 逻辑」——见 Open Questions Q1。

### D2. menu 项的平台分支：HTTP server mode 隐藏「在文件管理器打开」

**选**：组件内仅通过 `isTauriRuntime()`（`ui/src/lib/runtime.ts` 既有判定函数，内部即 `__TAURI_INTERNALS__ in window` 检查）单一判定为 `true` 才渲染 Finder 项；HTTP server mode (`?http=1`) 浏览器仅展示「复制路径」+「复制 Session ID」。
**否**：(a) 直接读 `__TAURI_INTERNALS__` 与 `isTauriRuntime()` 双判 —— 重复同源信号，徒增 spec 描述复杂度。(b) 始终渲染 Finder 项，调 plugin 失败再 toast —— HTTP mode 用户每次点击都 fail 是反产品体验。
**理由**：
- 三种实际运行模式：(1) Tauri 桌面（真 runtime → `isTauriRuntime() = true`）；(2) `?mock=1` 浏览器调试（mockIPC 注入 `__TAURI_INTERNALS__` → `isTauriRuntime() = true` + `tauriMock.ts::buildHandler` 已处理 `plugin:opener|open_path` 走 stub）；(3) `?http=1` 浏览器 server mode（无 `__TAURI_INTERNALS__` → `isTauriRuntime() = false`）。
- 对本 change：模式 (1)(2) 都展示 Finder 项 + 调 `openPath` API（mock 模式走 mock handler、真 Tauri 走真 plugin），模式 (3) 隐藏 Finder 项 — 单一 `isTauriRuntime()` 判定恰好正确分流。
- `ui/CLAUDE.md::测试基础设施陷阱` 警告「UI 代码不能用 `__TAURI_INTERNALS__` 判 mock vs real」—— 本 change 不区分 mock vs real（统一调 `openPath`），仅区分 http=1 vs (Tauri 或 mock)，所以 `isTauriRuntime()` 是合规用法。
- 与 `UnifiedTitleBar` 既有 macOS traffic-light 让位判定（`isMacOS && isTauriRuntime()`）同源。

### D3. 复制操作的反馈：inline 短暂态切换（无 toast）

**选**：菜单项点击后菜单立即关闭，触发器 `[⋯]` button 的 tooltip 区临时（1500ms）显示 `Copied` 文字反馈，然后恢复。
**否**：弹 toast 通知 / 全局 status bar / 无任何反馈。
**理由**：toast 重，违反 PRODUCT.md 「克制」；无反馈让用户疑虑是否生效。inline 在源头位置反馈是 IDE 共识（VS Code / Linear 复制按钮均如此）。失败路径（`navigator.clipboard.writeText` reject——多发生于 HTTP non-secure context）SHALL 切到 `Copy failed` 红字 1500ms。

### D4. 「在文件管理器打开」失败处理：plugin 调用 reject → menu 触发器 toast 错误态

**选**：调 `tauri-plugin-opener` 的 `open_path(cwd)`；catch reject → 在 menu 触发器位 inline 显示 `打开失败` 红字 1500ms（cwd 已删除 / plugin 拒绝 / 权限 deny 都走此路径）。日志通过 `tracing` (后端) 或 `console.warn` (前端) 留痕。
**理由**：与 D3 一致的 inline 反馈策略；不弹 dialog（PRODUCT.md 反 modal 反 popup）。删除 cwd 是低概率（本地 worktree 被清理 / 远程 ssh-remote 路径不存在），不值得专门 modal 引导。

### D5. `top-stats` 改 `flex-wrap: nowrap` + 移除 CWD 后保留时间精度降级

**选**：`flex-wrap: nowrap`（替换当前 `wrap`）；`LAST` 时间从秒级 `19:50:46` 改为分钟级 `19:50`（与 sidebar 一致）。
**理由**：
- nowrap 防御性约束未来 `top-stats` 再有人加变长项触发同类 wrap。CWD 是当前唯一变长项，删后剩余项均短数字，`nowrap` 安全。
- 时间精度降级与 sidebar `「刚刚 / 18m / 1h」` 信息密度对齐（PRODUCT.md「密度有层次」）；秒级精度未匹配任何用户故事。

### D6. menu 项数据约定：`detail.metadata.cwd` 数据通路保留

**选**：cwd 数据由 `cdt-api::get_session_detail` 返回的 `detail.metadata.cwd` 字段提供（既有不变）；菜单组件接 `cwd: string | undefined` prop。cwd 缺失时菜单仍按完整结构渲染——「在文件管理器中打开」与「复制工作目录路径」两项渲染 disabled 态（`text-muted` / `cursor: not-allowed` / `aria-disabled="true"` / 不响应 click），「复制 Session ID」项保持可用；menu 顶部 SHALL NOT 渲染额外提示文案（disabled 视觉态本身已传达，与 spec delta scenario「cwd 缺失时降级」一致）。
**理由**：
- 老 session jsonl 可能无 metadata.cwd（早期 Claude Code 版本写入字段不全），SHALL graceful degrade。
- 不动 IPC schema 避免 contract test 影响；不动 cdt-api 后端。
- 保持完整 menu 结构（不收缩到单项）让用户能预测菜单形态——同一 session detail 因 cwd 缺失而 menu 项数变化是反预测的。

### D-V1. menu 触发器：`.top-meta` 区 icon-only `[⋯]` overflow button，与 `[Context]` 并列

**选**：`MoreHorizontal` lucide icon-only button，复用 `.top-badge` 样式 token（13px icon / padding 6px 10px / radius 6px）；放在 `[Context N]` **左侧**（gap 8px，遵循 `.top-meta` 现有 gap）。
**否**：
- (a) icon + text "Workspace" / "Open" — text 增冗，与 `[Context 2]` 文字重复 + 语义不准（"打开什么"不清）。
- (b) 放在 h1 同行 — 长 title 时 ellipsis 行为被破坏（top-titles flex:1 min-width:0 机制依赖 h1 独占 column）。
- (c) ⓘ info popover 展示卡片 — 把"信息"和"动作"混了，回到包一切的反模式；菜单形态比卡片更对齐 IDE 习惯（VS Code / Linear / Notion 均用 `...` overflow menu）。
**理由**：
- PRODUCT.md「熟悉即效率」—— 桌面工具共识"右上 utility menu"。
- DESIGN.md `.top-badge` token 复用，不引入新控件类目。
- icon-only `⋯` 发现性低但本就是低频操作（session 级元操作），藏一层 menu 后面合理；`[Context]` 高频用 text+count，`[⋯]` 低频用 icon-only，**两者刻意区分视觉重量**反而更对。
- 长 title 时的布局：top-titles `flex: 1; min-width: 0`，top-meta `flex-shrink: 0`——h1 永远独占 top-titles 全宽并 ellipsis；`[⋯][Context]` 永远右贴；两者无空间竞争。

### D-V2. menu 展开方向：trigger 下方右对齐，箭头不画

**选**：dropdown overlay 锚定 trigger 下方（`top: trigger.bottom + 4px`），右对齐 trigger（避免 menu 溢出右窗边）；overlay 不画指向 trigger 的箭头（与 `Dropdown.svelte` 既有风格一致 —— 见 SettingsView 的 dropdown 案例）。
**理由**：
- top-bar 距窗顶仅 14px padding，向上展开会贴 TabBar 行底 border 视觉拥挤。
- 右对齐避免 trigger 靠近窗右时 menu 越界（`top-meta` 离窗右最多 24px padding，menu 需向左展）。
- 箭头是装饰性元素，PRODUCT.md「不做无意义点缀」—— Linear / Notion / VS Code 的 overflow menu 均无指向箭头。

### D-V3. menu 项视觉：lucide icon + label 文字 + 分隔线分组

**选**：每项 `[icon 14px][gap 8px][label 13px]`，padding 6px 12px；hover 态 background `surface-raised`，文本 `text` 主色；前两项（cwd 相关）一组，第三项（Session ID）以 1px `border-subtle` 分隔。
**否**：
- (a) 纯文字 menu 项 — 信息密度低，IDE menu 共识带 icon。
- (b) 全部一组无分隔 — 三项语义不同（cwd-folder / cwd-text / session-id），分组提示语义边界。
**理由**：与 `Dropdown.svelte` 既有 menu item 渲染范式一致（SettingsView dropdown 选项已用类似结构）。lucide icon 与 SessionDetail 顶 bar 其他 SVG（Context icon = file-text 风格）同源。

### D-V4. CWD 删除 + `LAST` 精度降级 → top-stats 视觉权重再平衡

**选**：CWD chip 移除后，`top-stats` 6 项变 5 项（AI / USER / TOOLS / TOK / LAST）；`LAST` 从 `19:50:46` 改 `19:50`（dropping seconds）。整行预估宽度由 ~700px 降至 ~280px，单行充裕。
**理由**：CWD 删除后 stats 真实变短，nowrap 安全；时间分钟级与 sidebar 一致密度。

### D-V5. 反馈呈现：trigger-anchored micro-toast，**不**改 trigger 宽度

**选**：action item click 后菜单立即关闭；toast 用 **`position: fixed` + portal 到 `document.body`** 渲染，几何锚定 trigger `getBoundingClientRect().bottom + 4px`、右对齐 trigger 右沿；`z-index: 200`（高于 `SearchBar` 的 100，避免 Cmd+F 搜索条同时可见时被遮挡）；fade-in 100ms ease-out / hold 1500ms / fade-out 150ms ease-out；token：`font-size: 11px; padding: 4px 8px; border-radius: 4px; background: var(--color-surface-overlay); border: 1px solid var(--color-border); color: var(--color-text-secondary); white-space: nowrap`；失败态切 `color: var(--color-danger)`，其余 token 不变。
**否**：
- (a) 菜单保持 open + item 内 inline 反馈：用户复制完通常想立刻回 detail 区，menu 滞留是噪音。
- (b) trigger 内 swap icon → text：button 宽度从 ~32px 涨到 ~64-72px，引发 `[Context]` 左移 → layout shift（违反 PRODUCT.md「实时但不闪烁」第 5 条 Design Principle）。
- (c) 全局 toast：重，违反 D3 既定的「不弹 toast」精神。
- (d) trigger 下方 8px `position: absolute` 子元素：与 `SearchBar`（top-bar 紧邻下方，line 762，Cmd+F 触发）视觉区域重叠，且依赖 `.top-bar` 不裁剪——脆弱。
**理由**：anchored micro-toast 是 Linear / Raycast / Vercel dashboard 复制反馈共识形态；语义为 source-attached confirmation 而非全局通知；与 `The Floating Is Affordance, Not Decoration Rule` 一致——仅在动作完成瞬间存在，1500ms 后消失。portal + fixed 让 toast 几何脱离父级 layout 影响，避开 SearchBar 重叠风险。

### D-V6. 过渡时长：100–150ms 区间，对齐 DESIGN.md::Section 5 motion baseline

**选**：
- trigger hover bg/border `transition: 120ms ease-out`
- menu overlay open/close 用 `opacity + translateY(-2px)` 150ms `cubic-bezier(0.16, 1, 0.3, 1)`（ease-out-quart）
- menu item hover bg `transition: 100ms ease-out`
- trigger feedback toast fade-in 100ms / fade-out 150ms（与 D-V5 锁死）

**理由**：对齐 DESIGN.md::Section 5 motion baseline「Hover/color/border 过渡通常 0.1–0.15s」+「Disclosure chevron 0.15–0.2s rotate」。SHALL 不动 `height/width/margin` 等布局属性触发重排。`prefers-reduced-motion: reduce` 下 SHALL 退化为即时 0ms 显隐。

## Visual Contract

> 本段第一稿。propose 阶段调用 `impeccable` skill 后回填 / refine（按 `.claude/rules/opsx-apply-cadence.md::Propose → Apply 之间` 第 2 项钩子）。

### Surface Decision

**入口位置**：`SessionDetail.svelte::.top-meta` 区（既有 toolbar），`[⋯] [Context N]` 顺序左→右并列。

**论证**：
- PRODUCT.md「Anti-references」明确反对"为'好看'重造标准控件"——`.top-meta` 已是 meta-action toolbar 容器，新增 button 在该区是延伸而非新区域。
- PRODUCT.md「Design Principles · 熟悉即效率」——桌面 IDE / Linear / Notion 共识"title 占左 + utility 占右"；本设计严格遵循。
- 拒绝候选 surface：(1) h1 同行（破坏长 title ellipsis）、(2) ContextPanel 内段（语义边界违反）、(3) ⓘ popover 卡片（动作信息混排），论证见 D-V1 / Non-Goals 第 4-5 项。

### Visual Layer

**复用 DESIGN.md Named Rules**（详见 `DESIGN.md` 主体段，本段仅引用名称稳定锚点）：

- **The Border Before Shadow Rule**：menu overlay 复用 DESIGN.md::Section 4「Floating overlay」标准 token —— `border-radius: 8px` / `1px solid var(--color-border-emphasis)` / `box-shadow: 0 4px 14px rgba(0,0,0,.12)` 浅色，深色主题切 `rgba(0,0,0,.25)`。trigger button 完全复用 `.top-badge` 既有 hover border 处理（**不**另起 token）。
- **The Status Owns the Color Rule**：menu 内部 SHALL NOT 引入装饰色 —— 成功反馈 `text-secondary`，失败反馈 `danger`，平时 menu items 走 neutral surface/text/border。trigger 默认 icon `text-muted`，hover `text` 主色，open 态背景 `surface-overlay`，全部对齐既有 `.top-badge` 体系。
- **The Tool Density Rule**：menu 项 icon `14px` / label `13px` / padding `6px 12px`，与 `SettingsView Dropdown` 既有 option row 密度对齐；menu overlay 总宽 SHALL 自适应内容（约 180–220px），SHALL NOT 强制 fixed width 防止中文/英文文案被裁剪。
- **The Floating Is Affordance, Not Decoration Rule**：menu overlay 与 trigger feedback micro-toast（D-V5）均**仅在动作进行 / 完成瞬间存在** —— menu open 是 affordance、close 即消失；toast 1500ms 后必消失。两者**不属于** chrome 持久结构，对齐「affordance not decoration」精神。
- **The No Decorative Glass Rule**：overlay SHALL NOT 加 `backdrop-filter: blur` —— 仅靠 `surface-raised` + `border-emphasis` + light shadow 解决层级，不走 glass affordance。

**新增视觉 token**：无（全部复用 `.top-badge` token + DESIGN.md::Section 4 既有 Floating overlay 标准 + Section 5 既有 motion baseline）。

### State Coverage

trigger / overlay / menu item / feedback 四类构件各自的状态机：

**trigger（`.top-badge`-styled icon button）**

| 状态 | 视觉 token | 实现位置 |
|---|---|---|
| `idle` | `background: transparent`；icon `var(--color-text-muted)` | `.top-badge` 默认 |
| `hover`（menu closed） | `background: var(--color-surface-raised)`；icon `var(--color-text)`；`border: 1px solid var(--color-border)` | `.top-badge:hover` |
| `focus-visible` | `outline: 2px solid rgba(59, 130, 246, .4)`（瞬时态，**允许** Focus Blue per DESIGN.md exception） | `.top-badge:focus-visible` |
| `active`（menu open） | `background: var(--color-surface-overlay)`；icon `var(--color-text)`；`border: 1px solid var(--color-border-emphasis)` | `.top-badge-active`（既有 mod 类复用） |
| `disabled` | n/a — trigger 始终可点击展开 menu；cwd 缺失的降级在 menu item 层处理（见下） | — |

**menu overlay**

| 状态 | 视觉 token | 实现位置 |
|---|---|---|
| `closed` | not rendered (SSR / Svelte `{#if open}` 控制) | `SessionMetaMenu.svelte` |
| `opening`（150ms） | `opacity: 0 → 1 + translateY(-2px → 0)`，`cubic-bezier(0.16, 1, 0.3, 1)` | D-V6 |
| `open` | `surface-raised` bg + `border-emphasis` + `0 4px 14px rgba(0,0,0,.12)` shadow + 8px radius | DESIGN.md Floating overlay 标准 |
| `closing`（150ms） | `opacity: 1 → 0`，ease-out | D-V6 |
| `loading / empty / error` | n/a — menu 内容是静态 action list，无异步加载 | — |

**menu item（每项独立状态）**

| 状态 | 视觉 token | 备注 |
|---|---|---|
| `idle` | `color: var(--color-text)`；`background: transparent` | — |
| `hover` | `background: var(--color-surface-raised)`；`color: var(--color-text)` | `transition: 100ms ease-out` |
| `focus`（键盘） | hover 同等态 + 文字 `var(--color-text)` 主色 | 方向键移动 |
| `active`（pressed 0–80ms） | `background: var(--color-surface-overlay)` | mouse down 即时反馈 |
| `disabled`（cwd 缺失，前两项降级） | `color: var(--color-text-muted)`；`cursor: not-allowed`；不响应 click / Enter / Space；Tab 跳过 | `aria-disabled="true"`、`tabindex="-1"` |

**feedback micro-toast（D-V5 trigger-anchored）**

| 状态 | 文案 | token |
|---|---|---|
| `hidden` | n/a | not rendered |
| `success`（Copied） | `已复制` | `color: var(--color-text-secondary)`；surface-overlay bg + border + 4px radius |
| `error-open` | `打开失败` | `color: var(--color-danger)`；其余同 success |
| `error-copy` | `复制失败` | 同 `error-open` |
| 显隐时长 | fade-in 100ms / hold 1500ms / fade-out 150ms（D-V6） | `prefers-reduced-motion: reduce` 退化即时显隐 |

**跨模式 / 跨主题**

| 模式 | 表现 |
|---|---|
| HTTP server mode | 「在文件管理器中打开」item 不渲染；分隔线随之消除（D2） |
| Tauri 桌面 mode | 三项完整渲染 + 分隔线 |
| 浅色主题 | 全部按上表 token |
| 深色主题 | overlay shadow 加深至 `rgba(0,0,0,.25)`；`text-muted/text/secondary` 自动走 `[data-theme="dark"]` token；其余 token 跟随 CSS 变量自动切换 |
| `prefers-reduced-motion: reduce` | trigger / overlay / item 所有 transition + animation 退化 0ms；menu open/close 即时显隐；toast fade 退化为即时显示 + 1500ms hold + 即时消失 |

### DESIGN.md delta plan

无 token / 组件提取需求 —— 本 change 的视觉构件全部由既有 token 组合，未生成需要沉淀的新设计原语。Archive 前 `/impeccable extract` 跑一次确认；若发现 menu 行为可抽象为通用 `MetaActionMenu` 给未来 session 操作复用，再独立 PR 提进 `lib/components/`。

## Risks / Trade-offs

- **[Risk] cwd 数据缺失老 session 用户体验降级** → menu 触发器仍可见但部分项 disabled；提示文案「无工作目录数据」让用户理解。降级路径有 unit test 覆盖。
- **[Risk] `Dropdown.svelte` 当前 API 不支持 action menu 模式（仅支持 value-select）** → apply 阶段开第一个子任务前评估「扩 Dropdown.svelte 加 menu mode」 vs 「新建 sibling `Menu.svelte`」；决策记录在 tasks.md。
- **[Risk] `tauri-plugin-opener::open_path` 在 Linux 某些 desktop env（i3 / sway 无 xdg-open default association）失败** → D4 已覆盖错误反馈路径；不阻塞功能上线。
- **[Risk] `navigator.clipboard.writeText` 在 HTTP non-secure context（用户用 IP 访问 server mode）reject** → D3 fail 路径覆盖；HTTP server mode 文档建议 localhost / HTTPS 使用，本 change 不阻塞。
- **[Trade-off] 删除顶 bar CWD 后失去"截图分享时一眼可识别 worktree"的旁路价值** → 已由 sidebar `#feat+xxx` 标签承担识别职责；用户截图分享详情页若需要 worktree 上下文，应连同 sidebar 一起截。
- **[Trade-off] 偏离原版 TS（原版无 cwd menu）** → `memory::feedback_align_with_original` 默认要求与原版对齐，本 change 显式偏离。原因：cwd 数据已存在 + 用户实操需求 + 原版无对应入口（不是抄漏，是新设计），proposal.md 已记录偏差理由。

## Open Questions

1. **Q1（已解决）**：`Dropdown.svelte` 仅支持 value-select 语义（`{ value, options, onChange }` API），**不**是 action menu pattern。决策：**不扩 `Dropdown`**（避免污染 select 语义），直接在 `SessionMetaMenu.svelte` 内部实现 action menu，借鉴 `Dropdown.svelte::placePopover` 几何代码（视口边界 / 翻转 / 右对齐）但**不**强行抽公共组件——按 Karpathy「3 行类似代码胜过过早抽象」，第二个 menu 用例（如 `SessionContextMenu` 重构）出现时再抽 `lib/components/Menu.svelte`。
2. **Q2（已解决）**：菜单项「复制 Session ID」**在本 change 内落地**——菜单分组 + 第三项是设计完整性的一部分；单独 PR 拆分会让中间态 menu 只有两项不带分隔线，违反设计完整性原则。
3. **Q3（已解决）**：HTTP server mode 下「在文件管理器打开」**隐藏**（不走 disabled+tooltip）——浏览器用户对该模式无感，disabled 项徒增视觉负担。若未来用户反馈"找不到那个选项"再评估改 disabled+tooltip。
4. **Q4（已解决）**：menu trigger icon 选 **`MoreHorizontal`**（⋯）—— 本 change 含「复制 Session ID」非 cwd 项，`Folder` 限定语义与第三项不匹配；`MoreHorizontal` 是「更多 session 操作」语义最通用入口（VS Code / Linear / Notion 共识）。
5. **Q5（已解决）**：反馈呈现走 **trigger-anchored micro-toast**（D-V5），不走 trigger 内 swap icon→text、不走全局 toast、不走菜单内 inline 反馈——避免 `[Context]` layout shift + 与 D3「不弹 toast」精神兼容（micro-toast 是 source-attached confirmation 不是全局通知）。

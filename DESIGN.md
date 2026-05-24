---
name: claude-devtools-rs
description: 可视化 Claude Code 会话执行的桌面工具设计系统
colors:
  surface: "#f9f9f7"
  surface-raised: "#f0efed"
  surface-overlay: "#e8e7e4"
  surface-sidebar: "#f2f1ef"
  text: "#1c1b19"
  text-secondary: "#6b6964"
  text-muted: "#9c9a95"
  border: "#e5e4e1"
  border-subtle: "#eeeded"
  border-emphasis: "#d5d3cf"
  accent-blue: "#3b82f6"
  accent-indigo: "#6366f1"
  success: "#15803d"
  success-bright: "#4ade80"
  danger: "#dc2626"
  danger-bright: "#f87171"
  warning: "#f59e0b"
  dark-surface: "#1e1e1c"
  dark-surface-raised: "#2a2a27"
  dark-surface-overlay: "#333330"
  dark-surface-sidebar: "#232321"
  dark-text: "#e8e6e1"
  dark-text-secondary: "#a8a5a0"
  dark-text-muted: "#6f6d68"
  dark-border: "#3a3a37"
  dark-border-emphasis: "#4f4e4a"
typography:
  title:
    fontFamily: "var(--font-sans)"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.4
  body:
    fontFamily: "var(--font-sans)"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  prose:
    fontFamily: "var(--font-sans)"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.65
  label:
    fontFamily: "var(--font-sans)"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "0.04em"
  mono:
    fontFamily: "var(--font-mono)"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "10px"
  bubble: "12px"
  pill: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  page: "48px"
components:
  sidebar:
    backgroundColor: "{colors.surface-sidebar}"
    textColor: "{colors.text}"
    width: "280px"
  tab-active:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    height: "40px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.xs}"
    padding: "6px 8px"
  dashboard-search:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.lg}"
    padding: "12px 16px"
  session-search:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    padding: "6px 10px"
  user-bubble:
    backgroundColor: "#e8e6df"
    textColor: "#3d3b36"
    rounded: "{rounded.bubble}"
    padding: "10px 14px"
  tool-item:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.xs}"
    padding: "6px 8px"
  switch-on:
    backgroundColor: "{colors.accent-indigo}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    width: "36px"
    height: "20px"
---

# Design System: claude-devtools-rs

## 1. Overview

**Creative North Star: “Quiet Debugging Workbench”**

claude-devtools-rs 的视觉系统是一个克制的桌面调试工作台：温暖中性色承载长时间阅读，细边框和低对比层级组织高密度信息，少量蓝色、靛紫、绿色、红色和琥珀色只在状态、选择和反馈中出现。界面应该让开发者相信这里记录的是可审计事实，而不是 AI 助手的营销包装。

产品默认处于桌面、IDE 相邻的使用场景：用户一边写代码一边回看 Claude Code 会话。设计应优先减少闪烁、跳动和装饰噪音，保留 sidebar、tab、split pane、command palette、inline disclosure 等成熟工具模式。

**Key Characteristics:**

- 暖中性色浅色主题与 Soft Charcoal 深色主题并行，跟随系统是一级主题模式。
- 高密度信息使用小字号、mono metadata、稳定边框和可折叠 disclosure 管理。
- 颜色稀缺，主要表达 active、focus、success、error、warning、ongoing、unread。
- 交互反馈轻量直接，通常 100–200ms，不做编排式动效。
- 桌面优先，macOS traffic lights、可拖拽 sidebar、多 pane 与键盘快捷键都是基础体验。

## 2. Colors

色彩策略是 Restrained：中性色占据绝大多数面积，语义色只在用户需要立即识别状态时出现。

### Primary

- **Workbench Warm Surface** (`--color-surface: #f9f9f7`): 主内容背景，用于会话详情、Dashboard 和设置页底色。
- **Raised Linen Surface** (`--color-surface-raised: #f0efed`): active tab、selected row、setting row、卡片 hover 等轻抬层级。
- **Overlay Ash Surface** (`--color-surface-overlay: #e8e7e4`): 图标容器、代码 header、次级容器。
- **Sidebar Warm Rail** (`--color-surface-sidebar: #f2f1ef`): 左侧导航和顶部 tabbar 的工具栏背景。

### Secondary

- **Focus Blue** (`#3b82f6` / code filename `#2563eb`): 搜索 focus ring、resize handle、链接、pin icon、临时 pulse、**ongoing/live 状态**。它是定位、交互焦点与实时进行中的统一颜色，不是装饰色。
- **Control Indigo** (`#6366f1`): 开关开启态、项目 dropdown check、少量设置控件。用于确认配置状态，不替代蓝色焦点语义。

### Tertiary

- **Execution Green** (`#15803d`, dark `#4ade80`): 成功工具结果、diff added、终端 prompt、**已完成**状态。绿色不表达"进行中"——ongoing/live 归 Focus Blue。
- **Failure Red** (`#dc2626`, dark `#f87171`): 错误工具结果、diff removed、通知 danger action。
- **Compaction Amber** (`rgba(245, 158, 11, ...)`): compacted content、warning banner 和需注意但非失败的系统状态。
- **Thinking Purple** (`#7c3aed`, dark `#a78bfa`): thinking block 与推理过程状态，不用于普通品牌强调。

### Neutral

- **Ink Text** (`--color-text: #1c1b19`): 主文本、标题、active label。
- **Secondary Graphite** (`--color-text-secondary: #6b6964`): metadata、说明文本、非 active 控件。
- **Muted Stone** (`--color-text-muted: #9c9a95`): 时间、辅助计数、disabled 或低优先级信息。
- **Hairline Border** (`--color-border: #e5e4e1`): 默认分隔线和卡片边框。
- **Emphasis Border** (`--color-border-emphasis: #d5d3cf`): active tab 下沿、focus-adjacent divider、重要分隔。
- **Soft Charcoal Dark Set** (`#1e1e1c`, `#2a2a27`, `#333330`, `#232321`): 深色主题不是纯黑，而是带暖灰倾向的低亮度层级。

### Named Rules

**The Status Owns the Color Rule.** 除 active、focus、语义状态、链接、diff、代码高亮以外，不新增彩色装饰。新增组件默认从 neutral surface/text/border 开始。

**The Warm Neutral Rule.** 禁止使用纯 `#000` / `#fff` 作为界面底色或主文本。浅色与深色主题都应保持轻微暖灰倾向。

**The Pairing Rule.** 新增语义色必须同时定义浅色与深色主题下的 bg/text/border 组合，并验证对比度。

**The Ongoing Owns Blue Rule.** 全应用所有"进行中/正在流式/live"指示器一律使用 Focus Blue 表达（Sidebar 行首 ongoing dot、SessionDetail 顶部 LIVE chip、AI thread 末端 live node、OngoingBanner）。Execution Green 只表达"已完成/成功"。颜色在跨页面之间必须一致——同一种状态在列表页是某色、在详情页换色，会让用户重新建立颜色映射，是审计场景的反模式。

**The Static-vs-Live Shape Rule.** "动态信号"与"识别用静态信号"必须**形态对立**，仅靠"参不参与动画"区分不够——相同颜色 + 相同 filled 形态 + halo 的多个静态点会被脉冲源"感染"，大脑把它们归入同一节律组、产生错觉脉冲。约定：动态 live 信号用 **circular spinner**（CSS border spinner：浅蓝静态环 + 蓝色顶弧 1.2s linear 恒速旋转）——恒速旋转是 IDE / 调试器工具的"白噪音"型 live 语言（VS Code / IntelliJ / GitHub Actions / cargo / pnpm 同款），眼睛快速适应、不持续抢戏，比周期性 dot ping 的 attention spike 更契合产品 register；所有静态识别点用 **outline 空心圆**（透明填充 + 1.25–2px 蓝边框、无 halo）。Sidebar OngoingIndicator、SessionDetail top-stat LIVE dot、ai-thread-node-live 一律 outline。SessionDetail 内允许 spinner 多于一个，但 SHALL 通过**尺寸 + 位置**分层 hierarchy：primary spinner = OngoingBanner（14×14 + 2px border + 标签条带，详情页底部独立条带）；secondary spinner = SubagentCard running 标记（10×10 + 1.5px border + header inline，无标签）。secondary spinner 不引入新的色相 / 节奏，仅靠尺寸缩档保持视觉权重低于 primary——避免之前用静态 outline 圆点时与 sa-status-done 静态对勾视觉同一类、看不出"事情正在发生"的体感问题。

**The One Live Signal Rule.** 单个 surface（Sidebar / SessionDetail / Dashboard）的 **primary** live 信号最多一个（按 `Static-vs-Live Shape Rule` 用 14×14 spinner + 标签条带承载）。**Secondary** live 信号（按 `Static-vs-Live Shape Rule` 缩档到 10×10 + inline 上下文）允许出现在 SessionDetail 的 SubagentCard 等 row-level 元素中——它们与 primary 在尺寸 / 位置上已天然分层，不会与 primary 抢主焦点（VS Code 同时存在 status bar spinner + editor tab spinner + explorer dirty mark 是同源做法）。其余非 ongoing 语义的识别点（top stat LIVE dot、ai-thread-node-live）继续用 outline 形态保持蓝色识别但不参与动画。Sidebar 因为 N 个 ongoing 会同时存在，N 个并发 spinner 会形成视觉噪音，因此 Sidebar OngoingIndicator 是**永远静态** outline 的。

**The Persistent Selection Is Quiet Rule.** 用户切换后**持续存在**的选中态（Sidebar 列表 / 文件树 / 持久导航行）SHALL NOT 使用 Focus Blue 或任何高饱和彩色作为视觉信号。Focus Blue 已被 `The Ongoing Owns Blue Rule` 分派给瞬时焦点（focus-visible ring、命令面板当前高亮项）和 ongoing/live；持久选中如用同色，会全程与 ongoing/live 信号在同一 viewport 视觉竞争，并使 sidebar 长期持有比 SessionDetail 主标题更高的视觉权重——违背"详情页是当前焦点，sidebar 是位置标记"的 PRODUCT.md 审计优先原则。

合规模式（任两条达 ≥3:1 即满足）：

- **Tonal layering**：背景使用 `--color-surface-overlay` 比 hover 的 `--tool-item-hover-bg` 加深一档。
- **结构性窄条 indicator**：左侧 1.5–2 px box-shadow inset，颜色用 `--color-text-secondary` 或 `--color-border-emphasis` 等暖中性色（非文本元素 WCAG 1.4.11 ≥3:1 即可）。
- **字重 contrast**：title 字重 600，与 hover 默认 400/500 拉开。

只有**瞬时**选中（临时导航高亮 / 不随页面切换持久保存的当前焦点项、tooltip 触发后 1–2 s 闪烁高亮目标）才允许 Focus Blue。

边界说明：

- **Tab active 不属于本规则反例。** Tab 选中已用"surface 抬升 + `inset 0 -2px 0` 下沿强调"做差异化（Section 5 App shell），不需要也不应叠加 indicator。
- **Pin / Hide 等专属状态指示器。** 使用各自专属颜色（pin indigo、hide opacity 衰减），不属于 selection 通道。
- **命令面板当前 hover 项是瞬时态。** 用户键盘上下移或鼠标悬停切换的临时导航高亮，不随页面切换持久保存，允许使用 Focus Blue（当前实现使用 `--color-surface-raised`，规则只是允许、不强制）。
- **Dashboard 项目卡 hover 不属于持久选中。** hover lift 是悬停反馈而非选中态。

边界说明（live signal 子规则，承接 `The One Live Signal Rule`）：

- **计入 live signal**：任何表达"正在执行 / 正在流式 / 正在加载远端数据 / 正在 streaming"的 infinite animation —— 包括 dot ping、shimmer sweep、circular spinner、`text-shimmer`、`progress-stripe`、`typing-indicator`（三点 / 三圈跳动）、`indeterminate progress bar`（宽度 / 位置 / 透明度持续变化）等。Subagent / Tool / 后台扫描的 running spinner 都计入。
- **不计入 live signal**：(a) 一次性短动画（≤ 2200ms，例如 anchor-pulse、card-pulse），用于交互反馈而非状态指示；(b) Skeleton placeholder（必须**静态** opacity 占位，禁用 shimmer，避免与真 live signal 竞争注意力）；(c) success / error / warning 静态语义指示器（不允许使用 ongoing blue —— 颜色已被分派给 success green / failure red / warning amber）；(d) `prefers-reduced-motion: reduce` 下所有 infinite animation 必须降级为静态形态。

**The Conflict Is Warning Not Error Rule.** 用户输入冲突的快捷键、表单未通过校验、可解决的临时阻塞 SHALL 用 warning（暖色 border + bg）而非 error red；error red（`--color-danger*`）仅保留给"系统已坏 / 操作不可恢复 / destructive 二次确认"。冲突是 actionable 反馈（"你需要解掉冲突"）而非错误（"系统坏了"），用 red 会过度报警、把日常配置操作误判为系统级故障；warning 暖色（amber 族）保留 attention 但不触发"红色 = 危险"的肌肉反射。Settings → Keyboard Shortcuts 录键 widget conflict 态、`ShortcutRow` 冲突 hint 行 SHALL 引用本规则。token：`--surface-conflict-bg` / `--border-conflict`（默认 alias 到 `--color-warning-bg` / `--color-warning-border`，浅 / 深 / system 三主题在 `app.css` 同步定义）。

## 3. Typography

**Display Font:** 不使用独立 display font。
**Body Font:** `var(--font-sans)`，默认 `-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', ... , sans-serif`。
**Label/Mono Font:** `var(--font-mono)`，默认 `ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace`。

**Character:** 字体策略是系统原生感和工具密度。标题不靠夸张字号建立品牌感，而靠位置、字重和信息结构建立层级；路径、token、时间、命令、diff 与日志使用 mono，帮助用户快速识别机器信息。

### Hierarchy

- **Title** (600, 16px, 1.4): Settings / Notifications 页面标题和少数区域主标题。
- **Pane Title** (500–600, 14px, 1.4): 会话标题、Dashboard 项目名、工具 label、AI header。
- **Body** (400, 14px, 1.5): 普通 UI 文案、列表标题、setting description。
- **Prose** (400, 14px, 1.65): AI markdown 内容，适合长段阅读；正文行长应尽量控制在 65–75ch，数据和代码例外。
- **Code / Output** (400, 12–13px, 1.5–1.55): Bash 输出、diff、read/write 文件内容、system pre。
- **Label** (600, 10–11px, uppercase optional, letter spacing 0.04em): 分组标题、状态 badge、section label、工具输出小标题。
- **Metadata** (400–500, 10–12px, mono where useful): 时间、token、duration、model badge、branch、session count。

### Named Rules

**The Tool Density Rule.** 产品 UI 不使用 fluid heading 或营销式 display scale。字号变化保持 1.125–1.2 左右，靠布局和字重补足层级。

**The Machine Information Rule.** 路径、命令、token、duration、diff gutter、branch 和日志输出优先使用 mono；自然语言说明不要滥用 mono。

## 4. Elevation

系统整体是 flat + tonal layering，不是 shadow-heavy。默认层级由 surface 色阶、1px border 和 inset/active stripe 表达；阴影只用于真正浮起在内容上方的元素，如 command palette、project dropdown、popover，或轻微 hover 的 Dashboard card。

常用 elevation 形态：

- **Tonal base:** `--color-surface`、`--color-surface-sidebar`、`--color-surface-raised`、`--color-surface-overlay`。
- **Hairline separation:** `1px solid var(--color-border)` 或 `--color-border-emphasis`。
- **Light hover lift:** Dashboard card hover 使用 `0 2px 8px rgba(0,0,0,0.06)`。
- **Floating overlay:** dropdown/popover 使用 8–12px radius、emphasis border、`0 4px 14–16px rgba(0,0,0,.12–.15)`。
- **Command layer:** command palette 使用 12px radius、`0 16px 48px rgba(0,0,0,.2)` 和 40% backdrop。

### Named Rules

**The No Decorative Glass Rule.** 不使用 blur/glass 作为默认层级。只有系统级 overlay 需要从背景分离时才允许加 backdrop 或强 shadow。

**The Border Before Shadow Rule.** 新增 panel、card、row 先用 surface + border + hover background 解决层级，只有浮层或明确 hover lift 才加 shadow。

## 5. Components

### App shell

- 主结构是 Sidebar + TabBar + Main / Split Pane。高度固定 `100vh`，所有滚动发生在内部 pane。
- Sidebar 可拖拽宽度，范围 200–500px；折叠时保留组件实例但宽度归零，避免虚拟列表和 ResizeObserver 闪烁。
- TabBar 高 40px，与 SidebarHeader 对齐；active tab 使用 surface 背景和 2px 底部强调线。
- macOS 隐藏 titlebar 场景必须给 traffic lights 留安全区，不能让 expand button 或 tab 占位冲突。

### Navigation rows

- Sidebar session item、project dropdown row、notification row、command result row 都遵循“透明默认，hover 显背景，active 用 raised surface”的模式。
- Active item 可以用细窄 2–3px indicator，但不要使用粗 side-stripe border。indicator 应属于选中态结构，不是卡片装饰。
- 分组 label 使用 10–11px uppercase、muted text、600 weight，避免大标题打断扫描。

### Buttons and icon controls

- 默认按钮是 ghost：透明背景、muted text、4–6px radius、hover 后显示 `--tool-item-hover-bg` 或 raised surface。
- Danger action 只有在 destructive 操作中使用红色；二次确认态应通过红色背景 + border 明确表达。
- Icon button 常用 28–36px hit area，图标 13–16px。所有 icon button 必须有 `aria-label`。

### Inputs and search

- Dashboard search 是大型入口：15px 字号、10px radius、surface 背景、emphasis border、`12px 16px` padding。
- Session `SearchBar` 是紧凑查找条：13px 字号、6px radius、贴合当前会话工具栏密度，不套用 Dashboard 大搜索样式。
- Dashboard search 的 focus ring 使用蓝色边框 + `0 0 0 3px rgba(59,130,246,.15)`；紧凑查找条保持当前 emphasis border focus 即可。
- Command palette、Session search、Dashboard search 都要支持键盘快捷键和 Escape 退出。

### Chat and execution trace

- User bubble 右对齐，max-width 75%，12px radius，`10px 14px` padding，带极轻 shadow。
- AI message 左对齐，max-width 95%，用左侧 2px thread border 表示连续执行轨迹。
- System message 使用 mono pre、system surface、`16px 16px 16px 4px` radius，max-width 85%。
- Compact block 使用 amber 语义，作为可展开系统状态，不作为普通提示卡。
- Ongoing / interruption 等状态应嵌入现有消息流或 slot，避免作为独立尾部节点造成滚动跳动。

### Tool items

- `BaseItem` 是工具、thinking、输出行的统一 disclosure：header `6px 8px`，4px radius，hover background，chevron 旋转 90° 表示展开。
- 展开内容使用左侧 2px border、`margin-left: 8px`、`padding-left: 24px`，与 AI thread 语言一致。
- Tool status dot 固定 6px：ok green、error red、pending/orphaned gray。
- Token、duration、model、language tag 均为小号 mono 或 badge，不应抢夺主内容权重。

### Code, diff, and output

- Code block 使用 `--code-bg`、`--code-border`、6px radius、12px mono、稳定 gutter。
- Diff added/removed 必须使用成对 bg/text/border token，不只靠文本颜色。
- Read/Edit/Write/Bash/Default viewer 保持同一 header、section label、copy/action button 语言。
- Syntax highlighting 的 `.hljs-*` token 只能在全局 `app.css` 维护，组件内不要局部覆盖。

### Cards and settings rows

- Dashboard project card 是可点击导航对象，不是营销卡片。使用 8px radius、16px padding、1px border、轻 hover lift。
- Setting row 使用 raised background、6px radius、左右布局；说明文案用 muted，控件靠右对齐。
- Toggle switch 尺寸 36×20，thumb 16×16，开启为 indigo，必须保留 `role="switch"`、`aria-checked`、focus-visible outline。

### Motion and transitions

- Hover / color / border 过渡通常 0.1–0.15s。
- Disclosure chevron 0.15–0.2s rotate。
- Switch thumb 0.2s ease-in-out。
- Dashboard active pulse 可使用 0.45s，但只用于再次点击当前项目这类瞬时反馈。
- 禁止动画 height、width 等布局属性来制造大范围重排；需要展开时优先保持内容结构稳定。

### Floating affordances

按需浮现的内容区导航按钮（如 SessionDetail「跳到最新消息」）。**affordance 不是状态**——按钮存在仅表示"动作可用"，不表达任何 ongoing / selection / success / warning 语义。

- **位置**：内容滚动容器的同级浮层（typically `.content-area` 内），`position: absolute`，距右下角各 `16px`；右侧浮层（如 ContextPanel）打开时通过 CSS class 切换 `right: calc(min(panel-width, 50vw) + 16px)` 让位（不读 JS 常量，与浮层 CSS 同源对齐；50vw 让窄屏不被推出可见区）。
- **形态**：icon-only，28×28 hit area，14px 图标，`6px` radius，`6 9l6 6 6-6` / `7 6l5 5 5-5` 等 lucide chevron 类形态承载方向语义；不带文字标签。
- **颜色**：`--color-surface-raised` bg + `1px solid --color-border-emphasis` + `--color-text-secondary` 图标色。**不允许**使用 Focus Blue / indigo / 任何语义色——见 `The Status Owns the Color Rule` + `The Floating Is Affordance, Not Decoration Rule`。
- **Elevation**：`0 2px 8px rgba(0,0,0,0.06)`（浅色主题，dark 加深至 `rgba(0,0,0,0.25)`）；hover 升至 `0 4px 12px rgba(0,0,0,0.08)`。引用 `The Border Before Shadow Rule` 的"浮层例外"——这是真浮层（脱离 flow，浮在内容上方），不是装饰阴影。
- **进出**：`opacity + translateY 8px`，进 200ms `cubic-bezier(0.16, 1, 0.3, 1)`（ease-out-quart）/ 出 150ms ease-out。不动 height/width/margin（避免 layout 重排）。
- **`prefers-reduced-motion: reduce`** 退化为即时显隐 + 程序滚动 `behavior: 'auto'`。
- **States**：default / visible / hover / focus-visible（`0 0 0 2px rgba(59,130,246,.15)` outline——这是**瞬时** focus，不与 ongoing blue 冲突）/ pressed (`scale(0.96)`) / programmatic-scrolling-suppressed 六态完备。
- **A11y**：必备 `aria-label`，`aria-hidden` 与 `tabindex` 跟随可见性切换（隐藏时屏蔽 a11y 树并退出 Tab 序列）；hover/focus tooltip 用 `title` 显示快捷键文案，按平台分流（macOS `⌘↓` / Win+Linux `Ctrl+End`）。

### Context menu

全应用右键菜单的浮层（capability `frontend-context-menu`）。所有 surface 共用 `AppContextMenu.svelte` items-driven 通用组件 + `use:contextMenu` Svelte action（`ui/src/lib/contextMenu.svelte.ts`）；菜单永远 portal 到 `document.body` 末尾，避免被 sidebar 虚拟滚动 / tab 横向滚动等 overflow 父容器 clip，也避免祖先 `transform / filter / contain` 创建 stacking context 把 z-index 围墙化。

- **形态**：单 column 文字菜单，min-width `200px` / max-width `var(--cm-max-width)`（默认 `320px`）；item `7px 12px` padding、`13px / 1.4 / 400` font，`role="menuitem"` + `tabindex="-1"`。Item 内部 flex 布局：`label`（`flex: 1` + `text-overflow: ellipsis` 末段截断）+ 右侧 `shortcut` **或** `chevron`（互斥，详下文 shortcut hint / submenu 子段）。
- **bg / border / radius**：`--color-surface` bg + `1px solid --color-border-emphasis` + `8px` radius + `4px` 内 padding；浅色与深色主题共用 token，无需独立 dark variant。
- **Elevation**：`0 4px 16px rgba(0, 0, 0, 0.15)`（与 Dropdown / popover 同档浮层）。引用 `The Border Before Shadow Rule` 的"浮层例外"——菜单是真浮层（脱离 flow），shadow 合规；**不**加 `backdrop-filter` —— 见 `The No Decorative Glass Rule`。
- **Item 状态**：default / hover / keyboard active / disabled（`aria-disabled="true"` 保留 a11y 可达性，opacity 0.45 / cursor not-allowed / Enter/Space no-op）/ danger（`--color-danger` 文字色 + 极淡红 hover bg）/ separator（`role="separator"`，`1px` `--color-border` 横线）/ submenu-trigger-active（拥有展开 submenu 的 parent item 持续保持 hover bg 锁定，让用户感知"哪个 item 当前对应已开 submenu"）。hover 与 keyboard active 共用 `--tool-item-hover-bg`；keyboard active 额外加 `2px solid rgba(59, 130, 246, 0.15)` outline 作为瞬时键盘焦点提示——这是 a11y 必需的合规模式，不违反 `The Persistent Selection Is Quiet Rule`（菜单 item 焦点是 transient ≤ 2s 决策窗，不是持久选中）。
- **键盘可达**：严格对齐 WAI-ARIA APG menu pattern。打开后 focus 立即进入第一个 menuitem；↑↓ 在所有 menuitem 间循环（**经过** `aria-disabled`，仅跳过 separator）；Enter / Space 触发 enabled item 的 action（`aria-disabled` no-op）；Esc 关闭并 focus 还回 trigger；鼠标 hover 同步 `activeIndex` 让键盘 / 鼠标焦点合一。submenu 键盘契约见 submenu 子段。
- **关闭触发**：document `mousedown` 外点 / Esc / window `blur` / 任意祖先 `scroll` / window `resize` 五种条件。**不**做 scroll reposition（菜单是 < 2s 决策浮层，重定位反而打断）。
- **Smart-select 防护**：`use:contextMenu` 在元素上挂 `mousedown` 监听，右键 `mousedown` 时若无选区调 `preventDefault` 阻止 WKWebView 自动选词；已有选区时不动选区（保留"先 drag-select 再右键"工作流给文本选区菜单消费）。
- **A11y**：菜单容器 `role="menu"` + `aria-orientation="vertical"`；Item `role="menuitem"`；分隔符 `role="separator"`；屏幕阅读器宣告"菜单 N 项 / 菜单项 X of N (已禁用)"。

#### Shortcut hint（行内右对齐）

item 右侧可选 shortcut 文本提示（如 `⌘C`、`⌘⇧E`），帮助用户在菜单上学习 / 回忆已注册全局快捷键——对齐 macOS / VS Code / Sublime 桌面工具惯例（`PRODUCT.md::Design Principles §2` 熟悉即效率）。

- **token**：color `--cm-shortcut-color`（alias `--color-text-muted`，自动跟主题）/ font `--cm-shortcut-font`（CSS font shorthand `11px var(--font-mono)`，组件 `font: var(--cm-shortcut-font)` + 单独 `font-weight: 400` 显式覆盖 shorthand reset）。三主题（`:root` / `[data-theme="dark"]` / `@media [data-theme="system"]`）同步显式声明保持契约表达一致。
- **位置**：item 行内 `flex` 布局右端，`flex-shrink: 0` + `margin-left: auto` + `padding-left: 16px`，`white-space: nowrap` 保证不换行。
- **可见条件**：仅有真实注册 shortcut 的 item 渲染（`item.shortcut` 字段非空）；无 shortcut 的 item 该位置留空，不渲染占位。
- **与 chevron 互斥**：item 同时具备 submenu 与 shortcut 时，渲染 chevron 优先（submenu 是结构性 affordance，比 hint 更必需）；这种组合不应在 menu-items factory 中产生——若产生属设计 bug。
- 引用 `The Machine Information Rule`（修饰键符号 mono）+ `The Status Owns the Color Rule`（hint 用 neutral muted，不引入彩色）。

#### Submenu（二级展开）

少数 surface 需二级菜单（如"在编辑器打开 ›" → VS Code / Cursor / Zed / Sublime；"在终端打开 ›" → Terminal / iTerm / Warp / Alacritty）。**优先策略**：menu-items factory 在 Settings 已设置默认应用时直接渲染单项 "在 {default} 打开"，仅 Settings 为"每次选择"时回退到 submenu，降低日常使用的认知开销（`PRODUCT.md::Design Principles §1` 审计优先）。

- **触发器视觉**：parent item 右侧渲染 `›`（U+203A）chevron，token `--cm-shortcut-color` + `font-size 14px / font-weight 500`；与 shortcut hint 互斥占位（同一槽位）。
- **形态**：与父菜单**完全相同**——bg `--color-surface` / border `1px --color-border-emphasis` / radius `8px` / shadow `0 4px 16px rgba(0,0,0,0.15)`。**禁止** tonal layering（不加深 bg）或加重 shadow——同语义浮层不分级，空间位移本身已提供视觉层次。这是 candidate **The Submenu Follows Parent Rule**（archive 前 `/impeccable extract` 决策是否提升为正式 Named Rule）。
- **递归实现**：`AppContextMenu.svelte` self-import 同组件递归渲染（同 stacking context，`position: fixed` 脱离父 box），比独立 `mount()` instance 更轻；通过 `data-cm-depth` 属性 hook 区分深度（仅作 hook 不施加额外 style）。
- **递归深度上限**：`canSpawnSubmenu` 检查 `depth < 2`——最多 2 级嵌套，超出忽略后续 submenu 字段（避免 menu-items 误声明产生菜单瀑布）。
- **进入交互**：鼠标 hover parent item `200ms` 后展开（`SUBMENU_HOVER_MS` 常量；hover 兄弟 item 时立即取消 timer 关旧开新）；键盘 ArrowRight 即时展开 + focus 进首项。**简化版**：暂不实现 submenu "安全三角"几何（鼠标对角穿越父 item 时仍会触发关闭），未来视用户反馈在后续 change 中补 polish；hover 离开 + 未进入 submenu 区域 → `150ms` 后关闭。
- **退出交互**：键盘 ArrowLeft 关闭 submenu + focus 回 parent；Esc 关闭整棵菜单树（`onCloseTree` 链向上传递）；item 触发 action 后 `rootCloseTree` 关整棵树。
- **定位**：默认右侧紧贴 parent menu 右边界展开；viewport 右边距不足以容纳 submenu 时翻转到 parent 左侧；垂直方向沿用 parent item 顶端。
- **暗色模式**：与浅色完全同 token，不做层级递进——`--color-surface` 在暗色主题已自动取 `#1e1e1c`，`--color-border-emphasis` 取 `#4f4e4a`，本身已与主背景区分；submenu 与父菜单视觉重叠时（viewport 边缘翻转致部分重叠）依靠 shadow + 1px border 提供层级。

#### Max-width 与路径中段截断

Phase 2 引入路径类 item（"在编辑器打开 ~/Rustrove…/contextMenu.svelte.ts"）+ shortcut hint，水平内容会超过原 220px 宽度。

- **`--cm-max-width: 320px`**（默认）：保证菜单不过宽——桌面窗口最小宽度 800px 时菜单 ≤ 40% 宽度。`min-width: 200px` 保证短 label item 仍达可点击宽度。
- **路径类 item 中段截断**：menu-items factory（`ui/src/lib/contextMenu/menu-items.ts::buildPathLabel`）在构造 item 时预处理路径——保留首段 home 前缀（`~/`）+ 尾段文件名最多 30 字符 + 中间 `…`，总长 ≤ 50 字符。CSS `text-overflow: ellipsis` 不能做中段截断，故必须 JS 层处理。`title` 属性挂完整原始路径，悬浮 tooltip 显示。引用 `The Machine Information Rule`（路径用 mono 辅助识别）。
- **末段截断 fallback**：未走 `pathLabel` 的长 label（如长 deeplink 标题）通过 `.cm-item-label { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis }` CSS 末段省略；shortcut / chevron `flex-shrink: 0` 不参与压缩。

### Floating affordances

按需浮现的内容区导航按钮（如 SessionDetail「跳到最新消息」）。**affordance 不是状态**——按钮存在仅表示"动作可用"，不表达任何 ongoing / selection / success / warning 语义。

- **位置**：内容滚动容器的同级浮层（typically `.content-area` 内），`position: absolute`，距右下角各 `16px`；右侧浮层（如 ContextPanel）打开时通过 CSS class 切换 `right: calc(min(panel-width, 50vw) + 16px)` 让位（不读 JS 常量，与浮层 CSS 同源对齐；50vw 让窄屏不被推出可见区）。
- **形态**：icon-only，28×28 hit area，14px 图标，`6px` radius，`6 9l6 6 6-6` / `7 6l5 5 5-5` 等 lucide chevron 类形态承载方向语义；不带文字标签。
- **颜色**：`--color-surface-raised` bg + `1px solid --color-border-emphasis` + `--color-text-secondary` 图标色。**不允许**使用 Focus Blue / indigo / 任何语义色——见 `The Status Owns the Color Rule` + `The Floating Is Affordance, Not Decoration Rule`。
- **Elevation**：`0 2px 8px rgba(0,0,0,0.06)`（浅色主题，dark 加深至 `rgba(0,0,0,0.25)`）；hover 升至 `0 4px 12px rgba(0,0,0,0.08)`。引用 `The Border Before Shadow Rule` 的"浮层例外"——这是真浮层（脱离 flow，浮在内容上方），不是装饰阴影。
- **进出**：`opacity + translateY 8px`，进 200ms `cubic-bezier(0.16, 1, 0.3, 1)`（ease-out-quart）/ 出 150ms ease-out。不动 height/width/margin（避免 layout 重排）。
- **`prefers-reduced-motion: reduce`** 退化为即时显隐 + 程序滚动 `behavior: 'auto'`。
- **States**：default / visible / hover / focus-visible（`0 0 0 2px rgba(59,130,246,.15)` outline——这是**瞬时** focus，不与 ongoing blue 冲突）/ pressed (`scale(0.96)`) / programmatic-scrolling-suppressed 六态完备。
- **A11y**：必备 `aria-label`，`aria-hidden` 与 `tabindex` 跟随可见性切换（隐藏时屏蔽 a11y 树并退出 Tab 序列）；hover/focus tooltip 用 `title` 显示快捷键文案，按平台分流（macOS `⌘↓` / Win+Linux `Ctrl+End`）。

### Named Rules（cross-cutting，与上面所有子节相关）

**The App Owns the Right-Click Rule.** 全应用右键事件统一由 `installGlobalContextMenuFallback()`（`ui/src/lib/contextMenu.svelte.ts`，`main.ts` 启动时注册）兜底 `preventDefault`，永远不漏 WKWebView / macOS 系统菜单（Reload / Look Up / Translate / Search with Baidu / Speech / Services 等）——破坏"克制、可信、工程化"调性。挂载自定义菜单 SHALL 走 `use:contextMenu={items | provider}` action（封装 portal mount + 键鼠 + a11y + smart-select 防护），禁止裸 `oncontextmenu` 单点实现以免日后散落 N 个不一致的菜单。例外：`<input>` / `<textarea>` / `[contenteditable="true"]` / `[data-allow-native-context]` 走系统菜单（保留输入便利与显式 opt-in 出口）。

**The Floating Is Affordance, Not Decoration Rule.** 浮层按钮仅在**动作语义**存在时显现（如长滚动列表的"回最新消息"、长 prose 的"回顶"、未读列表的"标记全读"）。一旦动作不再适用即隐去，**不**作为持久导航或装饰存在。这条规则与 `The Persistent Selection Is Quiet Rule` 互补：持久选中默认安静；瞬时 affordance 则可以浮起，但**仅在它能完成的那个动作仍然有意义时**。违反此规则的反例：常驻底部"返回顶部"按钮（即使页面没长到需要）、永远显示的"切换主题"浮 chip（应放入 Settings）。

**The Recorder Idle State Rule.** 录入类控件（录键 / 录手势 / 录笔画 / 录音 trigger）的 **idle 态** SHALL 是 neutral surface + 1px 弱 border + mono 当前值；**禁止**常驻 accent 边框、闪烁、infinite 动画。理由：录入控件常出现在 Settings 等长列表中，常驻 accent 会全程与真正的 ongoing/live 信号在 viewport 视觉竞争（违反 `The Persistent Selection Is Quiet Rule` 的同源逻辑——长期持有的视觉权重应让位给当前焦点 surface）。仅在用户主动触发录入（focus / click）时切到 **recording 态**：accent border + low-saturation tinted bg + secondary spinner（10–12px，按 `The Static-vs-Live Shape Rule` 缩档；spinner 标准 1.2s linear 恒速，与 OngoingBanner / SubagentCard 同节奏）。recording 态的 accent 选 `--color-accent-indigo`（与 `Settings switch on` 同源的 confirmed-state token）——**不**复用 `--color-accent-blue`：后者已被 `The Ongoing Owns Blue Rule` 分派给 ongoing/live + 瞬时 focus，混用会让"用户在配置"与"系统在跑"形态难分。**conflict 态**走 `The Conflict Is Warning Not Error Rule`。`KeyRecorderInput.svelte` 是首个引用案例。token：`--surface-recording-bg` / `--border-recording`（recording-bg 为 indigo 8%-14% 透明覆盖、border 为 indigo 实色，浅 / 深 / system 三主题在 `app.css` 同步定义）。

## 6. Do's and Don'ts

### Do

- 使用现有 CSS 变量，不在组件里散落新的 hex，除非同时补齐浅深主题 token。
- 新增 UI 优先复用 BaseItem、OutputBlock、DiffViewer、SettingsToggle、SearchBar、TabBar/Pane 模式。
- 保持桌面工具密度：小字号、明确 metadata、稳定边框、必要折叠。
- 对实时刷新路径使用 silent / in-place patch，保留旧内容直到新内容就绪。
- 给可点击控件补 `button` 语义、键盘行为、focus-visible 和 `aria-label`。
- 在浅色、深色、system 三种主题下同时检查状态色、diff、代码高亮和 hover。

### Don't

- 不要使用渐变文字、玻璃拟态、装饰性大阴影、hero metric、重复营销卡片网格。
- 不要把蓝色或靛紫色当普通装饰色铺满界面；它们属于 focus、link、**瞬时** selection、switch 等状态。**持久选中**（sidebar 列表 / 文件树等）不使用蓝色——见 `The Persistent Selection Is Quiet Rule`。
- 不要用粗 `border-left` / `border-right` 装饰卡片、列表项或提示。选中态如需 indicator，应是结构性的窄条，且只用于 active navigation。
- 不要新增 display font、fluid heading 或营销式大标题。
- 不要在工具输出、diff、日志中牺牲可复制性和等宽对齐。
- 不要新增只有鼠标可用的 clickable div/span；如果必须保留现状结构，需补齐 keyboard 和 ARIA。
- 不要让 loading / refresh 产生闪烁中间态，尤其是会话列表、SessionDetail 和 ContextPanel。

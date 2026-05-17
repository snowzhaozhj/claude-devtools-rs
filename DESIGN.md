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

**The One Live Signal Rule.** 单个 surface（Sidebar / SessionDetail / Dashboard）同时显示的**动态**（CSS `animation: ... infinite` 持续运行的）live 指示器最多一个；其余 live 指示器必须退为静态填充 + 静态 halo ring。SessionDetail 的 primary live 信号是 `OngoingBanner` 的 dot ping；其它位置（top stat LIVE dot、ai-thread-node-live、SubagentCard running 标记）保持蓝色识别但不参与脉冲。Sidebar 因为 N 个 ongoing 会同时存在，N 个并发脉冲会形成视觉噪音，因此 Sidebar OngoingIndicator 是**永远静态**的。

边界说明：

- **计入 live signal**：任何表达"正在执行 / 正在流式 / 正在加载远端数据 / 正在 streaming"的 infinite animation —— 包括 dot ping、shimmer sweep、circular spinner、`text-shimmer`、`progress-stripe`、`typing-indicator`（三点 / 三圈跳动）、`indeterminate progress bar`（宽度 / 位置 / 透明度持续变化）等。Subagent / Tool / 后台扫描的 running spinner 都计入。
- **不计入 live signal**：(a) 一次性短动画（≤ 2200ms，例如 anchor-pulse、card-pulse），用于交互反馈而非状态指示；(b) Skeleton placeholder（必须**静态** opacity 占位，禁用 shimmer，避免与真 live signal 竞争注意力）；(c) success / error / warning 静态语义指示器（不允许使用 ongoing blue —— 颜色已被分派给 success green / failure red / warning amber）；(d) `prefers-reduced-motion: reduce` 下所有 infinite animation 必须降级为静态形态。

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
- 不要把蓝色或靛紫色当普通装饰色铺满界面；它们属于 focus、link、selection、switch 等状态。
- 不要用粗 `border-left` / `border-right` 装饰卡片、列表项或提示。选中态如需 indicator，应是结构性的窄条，且只用于 active navigation。
- 不要新增 display font、fluid heading 或营销式大标题。
- 不要在工具输出、diff、日志中牺牲可复制性和等宽对齐。
- 不要新增只有鼠标可用的 clickable div/span；如果必须保留现状结构，需补齐 keyboard 和 ARIA。
- 不要让 loading / refresh 产生闪烁中间态，尤其是会话列表、SessionDetail 和 ContextPanel。

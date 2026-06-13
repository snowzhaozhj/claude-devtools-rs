## Context

会话详情流（`SessionDetail.svelte`）里，AI 消息 chunk 容器（`:1098`）挂了 `use:contextMenu={() => buildAssistantMessageItems(...)}`，但它内部的四类工具展开块是裸 `<div class="prose lazy-md">`（外层 `BaseItem` 包装），**没挂自己的右键菜单**：

- slash/SKILL 指令（`item.type==="slash"`，`:1209`，复制源 `item.slash.instructions`）
- Output 块（`item.type==="output"`，`:1276`，复制源 `item.text`）
- Thinking 块（`item.type==="thinking"`，`:1262`，复制源 `item.text`）
- User message 块（`item.type==="user_message"`，`:1289`，复制源 `item.text`）

右键这些子块时事件冒泡到父 AI chunk 菜单，而该菜单复制项由 `aiChunkToMarkdown`（`markdown.ts:55`）生成、只取 `kind==="text"` 步骤、明确排除 thinking/tool/slash——所以右键这些块复制到的是 AI 文字正文，不含该块内容。

现有 spec `session-display::消息 chunk 右键菜单` 已有 Scenario「子元素右键不触发消息层菜单」，但它只覆盖**已挂 `use:contextMenu` 的工具块**（`BashToolViewer` 等）；这四类裸 prose 块从未挂过菜单，正是覆盖缺口。

基础设施已齐备：`use:contextMenu` action 的 `handleContextMenu` 已 `e.stopPropagation()`（阻止冒泡到父 chunk），`handleMouseDown` 已做 WKWebView smart-select 防护，选区融合由 `appendSelectionCopyIfAny` 处理，`stripMarkdownFormatting`（纯文本）已存在于 `markdown.ts`。本 change 只需把现成 factory 模式复制到一个新 surface。

## Goals / Non-Goals

**Goals:**

- 右键四类工具展开块时，弹出**该块**的复制菜单（复制纯文本 / 复制为 Markdown / 有选区时复制选中文本），不冒泡到整条 AI 消息菜单。
- 复用现有 context menu 视觉契约与菜单措辞，不引入新用户预期。
- 零新增按钮、零新增组件、零 IPC/后端改动。

**Non-Goals:**

- **不**新增任何可见/浮现复制按钮（经 impeccable shape 与用户确认排除）。
- **不**改动代码块（`.code-block-copy`）与 OutputBlock（`.copy-float`）现有 hover 复制按钮——它们是更细粒度的子元素复制，与右键整块互补。
- **不**触及 `CopyButton` / `mode` prop 的 spec 漂移（既有技术债，另开 issue）。
- **不**给 subagent / teammate / workflow card 等已有专属交互的块加菜单（本 change 只覆盖四类裸 prose 块）。

## Decisions

### D1：新增单个通用 factory `buildMarkdownBlockItems(text, ctx)` 而非每类块各一个 factory

四类块的复制语义完全相同（一段 markdown 源文本 → 纯文本 + Markdown 两项），用一个通用 factory 接受 `text` 参数即可，避免 `buildSlashItems` / `buildOutputItems` / `buildThinkingItems` 四个近乎重复的函数。factory 内：`appendSelectionCopyIfAny(items, ctx)` → 「复制纯文本」(`stripMarkdownFormatting(text)`) → 「复制为 Markdown」(原文 `text`) → `finalizeWithSeparators`。

**Alternative considered**：每类块独立 factory（对齐现有 `buildBashToolItems` 等按 surface 拆分的风格）。否决：那些 factory 因复制源结构不同（exec.input / exec.output / diff）才需要拆；这四类块复制源同构（都是一段 markdown string），拆分只增重复。

### D2：复制源直接用块的原始 text，纯文本走 `stripMarkdownFormatting`

slash instructions / output text / thinking text 字段本就是未经 marked 渲染的 markdown 源。「复制为 Markdown」= 原文；「复制纯文本」= `stripMarkdownFormatting(text)`（复用 `markdown.ts` 既有最小 strip）。不新增数据提取路径、不反向 HTML、不新增 IPC。

### D-V1：复制入口选「右键菜单落点修正」而非「hover 浮现块级按钮」

用户原话：右键这个块就应该复制这个块。该方案零新增按钮、零视觉噪音，且把"复制能力"统一收敛进既有右键菜单语言。引用 `DESIGN.md::The App Owns the Right-Click Rule`（自定义菜单统一走 `use:contextMenu`，禁止裸 `oncontextmenu`）与 `DESIGN.md::The Floating Is Affordance, Not Decoration Rule`（不为可发现性引入常驻/浮现装饰）。

**Alternative considered**：给 `BaseItem` 加 `copyText` prop + 展开内容区右上角 overlay 复制按钮（复用 OutputBlock `.copy-float` 语言）。否决理由（用户驱动）：与代码块/OutputBlock 子元素复制按钮叠加时同一 hover 区会出现两级复制按钮，视觉噪音 + 重复；且右键复制块更贴合用户直觉。

### D-V2：菜单措辞复用现有「复制纯文本 / 复制为 Markdown」

与 `buildAssistantMessageItems` / `buildUserMessageItems` 完全一致的措辞，用户在整条消息右键时已建立这套预期，下沉到子块不引入新词汇。引用 `DESIGN.md::Context menu` 视觉契约（无 icon copy 项、separator 分组、shortcut hint）+ product register 的 earned familiarity。

### D3：`use:contextMenu` 挂在展开后的 `.prose.lazy-md` 原生 div 上，不挂 BaseItem 组件

`use:` action 只能落到原生 DOM 元素，不能直接挂 Svelte 组件（`BaseItem`）。这四类块的 prose 内容是 `BaseItem` 的 children snippet 里的 `<div class="prose lazy-md">`，是原生 DOM——`use:contextMenu` 挂在它上面。后果与边界：

- **命中区域 = 展开后的内容区**（prose 块本身），不含 header 行。右键 header 落到父 AI chunk 菜单——可接受：header 是 disclosure 控件不是内容，且用户要复制的是内容。
- **折叠态 prose 不渲染**（`BaseItem` 仅 `isExpanded && children` 时渲染 content），故折叠态无块菜单——合理：内容不可见时复制无意义（呼应 `The Floating Is Affordance, Not Decoration Rule`）。
- **slash 无 instructions** 时 prose 不渲染，自然无菜单。
- 一个元素可同时有 `{@attach attachMarkdown(...)}` 与 `use:contextMenu`，互不干扰。

**Alternative considered**：在调用侧用 `<div use:contextMenu>` 外包整个 `<BaseItem>`。否决：命中区域会扩到 header + 折叠态，与"复制展开内容"语义不符，且 header 右键会与父 chunk 菜单争夺。

## Visual Contract

### Surface Decision

复制入口落在**既有右键菜单 surface**（`AppContextMenu` portal 浮层），不引入新 surface、不新增按钮。论证见 `D-V1`，链回 `PRODUCT.md::Anti-references`（不为"好看"重造标准控件）与 `PRODUCT.md::Design Principle 2`（熟悉即效率，沿用 inline disclosure + 右键菜单成熟模式）。

### Visual Layer

无新增视觉元素。四类块的右键菜单完全复用 `frontend-context-menu` 既有视觉规格——引用 `DESIGN.md::Context menu`（单 column 文字菜单 / `--color-surface` bg / `--color-border-emphasis` border / 8px radius / `0 4px 16px` shadow）、`DESIGN.md::The App Owns the Right-Click Rule`、`DESIGN.md::Shortcut hint`（`⌘C` 等宽 muted 右对齐）。

### State Coverage

| 状态 | 表现 |
|---|---|
| 右键块（无选区） | 弹「复制纯文本」+「复制为 Markdown」两项 |
| 右键块（有选区） | 首项插入「复制选中文本」(`⌘C`)，下接两项 |
| 复制成功 | 沿用菜单项 `feedback: { label: "已复制!" }` → 600ms 后关闭 |
| 复制失败 | 沿用 `copyToClipboard` 静默降级 |
| slash 无 instructions | 该 `BaseItem` 不可展开、无 content（已有逻辑），不挂菜单或菜单为空不弹 |
| 键盘 ContextMenu 键 / Shift+F10 | 沿用 `use:contextMenu` action 的 `handleKeyDown` 定位到块 bbox 中心 |
| `prefers-reduced-motion` | 菜单本身无 infinite 动画，无需额外处理 |

### DESIGN.md delta plan

无新增 token / 组件，无需 `/impeccable extract`。本 change 纯属在既有 surface 复用既有视觉契约。

## Risks / Trade-offs

- **可发现性依赖右键** → 缓解：本产品全面采用右键菜单（`DESIGN.md::The App Owns the Right-Click Rule`），用户为工程师群体，VS Code 式右键复制是肌肉记忆；且代码块/OutputBlock 的可见复制按钮仍兜底细粒度复制。
- **嵌套 `use:contextMenu` 的事件冒泡** → 子块 action 的 `stopPropagation` 已阻止冒泡到父 AI chunk（现有 action 行为，已有 Scenario 覆盖工具块路径）；新增四类块走同一机制，风险低。e2e SHALL 验证「右键 output 块只弹 output 菜单、不弹整条消息菜单」。
- **空 text 块** → `buildMarkdownBlockItems("", ctx)` 返回 `[]`（与 spec/tasks 一致），`openMenu` 收到空 items 直接不弹菜单；slash 块在 `instructions` 为空时 prose 不渲染、自然不挂 provider。三处（design / spec / tasks）对空 text 行为统一为"返回空数组 → 不弹菜单"。

## Open Questions

无。

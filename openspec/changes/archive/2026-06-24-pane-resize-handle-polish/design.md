## Context

`PaneResizeHandle.svelte` 是多 Pane 分屏中相邻 pane 之间的拖拽分隔条。当前实现是 6px 透明区域 + hover/active 时 `--color-border-emphasis` 背景色，无常驻视觉分隔、无 ARIA 语义、无键盘操作路径。Sidebar 的 resize handle 已有完整实现（`role="separator"` + 键盘 + oklch accent-blue hover），两者一致性缺口在 impeccable critique 中被标记为 P2。

参照：`Sidebar.svelte` lines 1175-1209（ARIA + 键盘）、lines 1776-1794（CSS）。

## Goals / Non-Goals

**Goals:**
- 加常驻 1px 分隔线让 pane 边界始终可见
- 补齐 ARIA 语义（WAI-ARIA Window Splitter pattern）和键盘 resize
- 统一 hover/active/focus-visible 高亮色到 oklch accent-blue，与 Sidebar handle 一致

**Non-Goals:**
- 双击还原等宽（P3，留后续）
- 触摸设备优化（桌面工具产品）
- 拖拽时实时 ARIA `aria-valuenow` 通知（鼠标操作期间不触发 AT 通知）

## Decisions

### D1：常驻分隔线用 `::after` pseudo-element 而非 `border-left`

**选定**：`::after` 1px `--color-border-emphasis`，绝对定位居中于 5px handle 内。

**候选**：
- (A) `border-left: 1px` 在 PaneView 上 → 需要改 PaneView 的 CSS + 处理第一个/最后一个 pane 边界条件
- (B) `::after` 在 handle 内 → 自包含，不影响外部布局

**取舍**：B 更内聚，分隔线与 handle 是同一个视觉 affordance 的两态（idle 见线、hover 见面），不该散到两个组件。

### D2：hover 高亮色统一为中性灰

**选定**：`color-mix(in oklch, var(--color-border-emphasis) 60%, transparent)`，Sidebar handle 同步改为一致。

**候选**：
- (A) 保持 neutral `--color-border-emphasis` → 低调
- (B) 统一用 accent-blue → 视觉统一但太抢眼

**取舍**：resize handle 是 secondary layout affordance，不是 primary navigation。accent-blue 在产品中用于 focus/selection 等状态语义——resize handle hover 用蓝色太突兀（用户反馈确认）。中性灰保持克制、可信、工程化的产品调性。

### D2b：反转 D2 原方案（accent-blue → neutral gray）

**原 D2** 选定 accent-blue 与 Sidebar 统一。apply 阶段视觉验证后用户反馈"蓝色太突兀"——resize handle 不应与 focus/selection 等语义色竞争视觉权重。反转为 neutral gray 并同步修改 Sidebar handle（`Sidebar.svelte`），两处统一用 `--color-border-emphasis` 60% 透明度。

### D3：键盘 resize 步长用 fraction 而非 px

**选定**：常规步长 0.05（总宽的 5%），Shift 加速 0.15（15%）。Home/End 分别设为 `MIN_FRACTION` 和 `combined - MIN_FRACTION`。

**候选**：
- (A) 像素步长 10px / 40px（Sidebar 模式）→ 需要在 keydown handler 中读 containerEl 宽度换算 fraction
- (B) 直接用 fraction 步长 → 与 `resizePanes` 接口天然对齐，无需 DOM 查询

**取舍**：PaneResizeHandle 操作的底层接口是 `resizePanes(paneId, newFraction)`，fraction 步长避免 px↔fraction 换算。0.05 在典型 1280px 宽度下约 64px，接近 Sidebar 的 40px shift 步长体感。

### D4：`aria-valuemin/max/now` 语义

- `aria-valuemin`：`MIN_FRACTION * 100`（即 10，表示 10%）
- `aria-valuemax`：`(1 - MIN_FRACTION * (paneCount - 1)) * 100`（随 pane 数动态计算）
- `aria-valuenow`：`Math.round(leftPane.widthFraction * 100)`

值域转为百分比整数（0-100 范围），对屏幕阅读器更友好。

## Risks / Trade-offs

- **[低]** `color-mix(in oklch, ...)` 在 Safari < 16.4 不支持 → Tauri WKWebView 绑定系统 WebKit，macOS 13+ 已支持；fallback `rgba(59, 130, 246, 0.5)` 已在 Sidebar handle 验证过可用
- **[低]** focus-visible outline 在极窄 handle（5px）上不美观 → 用 `outline: none` + 整条高亮色背景作为 focus 指示，与 Sidebar handle 一致

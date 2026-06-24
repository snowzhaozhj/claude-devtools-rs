## MODIFIED Requirements

### Requirement: Pane 宽度拖拽 resize

相邻 pane 之间 SHALL 渲染一个可拖拽的 `PaneResizeHandle`。拖动 handle SHALL 仅调整相邻两个 pane 的 widthFraction，其他 pane 宽度不变。单个 pane 的 widthFraction SHALL clamp 到 `[0.1, 1 - 0.1 * (n - 1)]`（n = 总 pane 数）。

PaneResizeHandle SHALL 在 idle 态展示 1px 常驻分隔线（`--color-border-emphasis`），hover/active/focus-visible 态 SHALL 切换为整条半透明中性灰高亮（`color-mix(in oklch, var(--color-border-emphasis) 60%, transparent)`），此时分隔线 SHALL 消隐。视觉语言 SHALL 与 Sidebar resize handle 一致。

PaneResizeHandle SHALL 具有 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label="拖动调整面板宽度"` ARIA 语义。`aria-valuemin` SHALL 为 `MIN_FRACTION * 100`，`aria-valuemax` SHALL 为 `(1 - MIN_FRACTION * (paneCount - 1)) * 100`（随 pane 数动态），`aria-valuenow` SHALL 为 `Math.round(leftPane.widthFraction * 100)`。

PaneResizeHandle SHALL 支持键盘 resize：ArrowLeft 减少 leftPane 的 widthFraction（步长 0.05），ArrowRight 增加（步长 0.05），Shift 修饰键 SHALL 加速步长至 0.15。Home SHALL 设 leftPane fraction 为 `MIN_FRACTION`，End SHALL 设为 `combined - MIN_FRACTION`（combined = leftPane + rightPane 的 widthFraction 之和）。键盘操作 SHALL 经过与拖拽相同的 `resizePanes` clamp 逻辑。

#### Scenario: 拖动相邻 handle
- **WHEN** 用户在 pane i 与 pane i+1 之间的 handle 上拖拽
- **THEN** pane i 的 widthFraction SHALL 跟随鼠标位置更新，pane i+1 SHALL 补差额以保持二者之和不变

#### Scenario: clamp 到最小宽度
- **WHEN** 拖拽使 pane i 的 fraction 将小于 0.1
- **THEN** 系统 SHALL 把 pane i 的 fraction clamp 到 0.1 并停止进一步缩小

#### Scenario: 不影响非相邻 pane
- **WHEN** 在 pane 1 和 pane 2 之间 resize
- **THEN** pane 3、pane 4 的 widthFraction SHALL 保持不变

#### Scenario: 常驻分隔线
- **WHEN** resize handle 处于 idle 态（无 hover / 无 focus / 无 drag）
- **THEN** handle SHALL 展示 1px 常驻分隔线（`--color-border-emphasis`），cursor 为 `col-resize`

#### Scenario: hover/active/focus-visible 视觉反馈
- **WHEN** 鼠标悬停、拖拽中、或键盘 focus-visible resize handle
- **THEN** handle SHALL 展示整条半透明 accent-blue 高亮，idle 态的 1px 分隔线 SHALL 消隐
- **AND** 高亮色 SHALL 为 `color-mix(in oklch, var(--color-border-emphasis) 60%, transparent)`，与 Sidebar resize handle 一致

#### Scenario: 键盘 resize
- **WHEN** handle 获焦且用户按 ArrowLeft
- **THEN** leftPane 的 widthFraction SHALL 减少 0.05（Shift 修饰时减少 0.15），rightPane 补差额
- **AND** ArrowRight SHALL 增加 0.05（Shift 修饰时增加 0.15）
- **AND** Home SHALL 设 leftPane fraction 为 MIN_FRACTION
- **AND** End SHALL 设 leftPane fraction 为 combined - MIN_FRACTION

#### Scenario: ARIA 语义
- **WHEN** PaneResizeHandle 渲染
- **THEN** 元素 SHALL 具有 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label="拖动调整面板宽度"`
- **AND** `aria-valuenow` SHALL 反映 leftPane 的当前 widthFraction 百分比（四舍五入整数）

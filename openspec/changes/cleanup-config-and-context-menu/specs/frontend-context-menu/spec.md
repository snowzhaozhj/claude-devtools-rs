# frontend-context-menu Spec Delta

## MODIFIED Requirements

### Requirement: SessionContextMenu / TabContextMenu 重构兼容

侧栏会话项与 tab 项的右键菜单 SHALL 改用通用菜单组件实现，但外部 API（props / 调用面 / 触发位置）SHALL 保持兼容。侧栏 / tab bar 内现有 oncontextmenu 直挂调用 SHALL 在重构后**改用**右键 action 形式（让动态 items 计算更内聚），但用户可见行为（菜单项内容、文案、顺序、动作语义、触发位置、复制 item 反馈短显示后关闭）一一对齐。

#### Scenario: Sidebar 会话项右键菜单回归

- **WHEN** 重构完成后，用户在侧栏会话项上右键
- **THEN** SHALL 弹出含 在当前标签页打开 / 在新标签页打开 / 在新 Pane 打开 / 置顶/取消置顶 / 隐藏/取消隐藏 / 复制 Session ID / 复制恢复命令 的菜单
- **AND** 各 item 顺序、文案、动作 SHALL 与重构前一致
- **AND** 复制 item 触发后 SHALL 显示"已复制!"反馈 600ms 后关闭

#### Scenario: TabBar 标签项右键菜单回归

- **WHEN** 重构完成后，用户在 tab 项上右键
- **THEN** SHALL 弹出与重构前内容一致的关闭 / 移到新 pane 类菜单
- **AND** 各 item 动作 SHALL 与重构前一致

### Requirement: AppContextMenu submenu 渲染

通用菜单 SHALL 扩展支持 submenu 渲染：检测 item.submenu 非空时挂 chevron + 进入 hover 状态后短延迟弹出二级菜单（具体阈值见 Scenario，同样通过 mount 到 document.body）；ArrowRight SHALL 即时打开 submenu + focus 进 submenu 首项；ArrowLeft SHALL 关闭 submenu + focus 还回 parent；Esc SHALL 关闭整棵菜单树；submenu 视觉规格与父菜单完全相同（同 bg / border / radius / shadow），不做层级递进。submenu 渲染深度 SHALL 限制为 ≤ 2。

#### Scenario: hover 短延迟打开 submenu

- **WHEN** 用户鼠标 hover 含 submenu 的 item 持续 200ms
- **THEN** SHALL 在 parent item 右侧弹出 submenu 浮层
- **AND** parent item SHALL 保持 active bg 锁定直到 submenu 关闭
- **AND** viewport 右边距不足时 submenu SHALL 翻转到左侧展开

#### Scenario: ArrowRight 即时打开 + focus 进 submenu

- **WHEN** 用户键盘导航至含 submenu 的 active item，按 ArrowRight
- **THEN** submenu SHALL 立即弹出（无 200ms 延迟）
- **AND** focus SHALL 进入 submenu 首项

#### Scenario: ArrowLeft 关闭 submenu + focus 回 parent

- **WHEN** submenu 已打开且 focus 在 submenu 内某项，用户按 ArrowLeft
- **THEN** submenu SHALL 关闭；focus SHALL 还回 parent item

#### Scenario: Esc 关闭整棵菜单树

- **WHEN** submenu 已打开，用户按 Esc
- **THEN** submenu 与 parent 菜单 SHALL 同时关闭；focus SHALL 还回 trigger 元素

#### Scenario: submenu 视觉与父菜单完全一致

- **WHEN** submenu 渲染
- **THEN** SHALL 复用父菜单的 bg / border / radius / padding / shadow token
- **AND** SHALL **不**加深 bg 或追加额外 shadow（遵守 `DESIGN.md::§1 Overview` flat + tonal layering 原则）

#### Scenario: 渲染深度上限 2

- **WHEN** 调用方传入 nested submenu 三层以上
- **THEN** 菜单组件 SHALL 在 depth 2 后忽略后续 submenu 字段
- **AND** depth 2 的 item 即使含 submenu 也按 leaf item 渲染

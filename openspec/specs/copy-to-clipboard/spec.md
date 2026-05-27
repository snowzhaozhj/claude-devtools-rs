# copy-to-clipboard Specification

## Purpose
TBD - created by archiving change add-copy-button. Update Purpose after archive.
## Requirements
### Requirement: CopyButton 组件提供 inline 和 overlay 两种渲染模式

CopyButton 组件 SHALL 接受 `text`（要复制的字符串）和 `mode`（`"inline"` | `"overlay"`）两个必需 prop。

#### Scenario: inline 模式渲染为常规按钮
- **WHEN** CopyButton 以 `mode="inline"` 渲染
- **THEN** SHALL 在文档流内渲染为普通按钮（不绝对定位）
- **AND** SHALL 始终可见（不依赖 hover）

#### Scenario: overlay 模式渲染为悬浮按钮
- **WHEN** CopyButton 以 `mode="overlay"` 渲染
- **THEN** SHALL 绝对定位于父容器右上角
- **AND** 默认 `opacity: 0`，父容器 hover 时 `opacity: 1`
- **AND** 父容器 SHALL 具有 `position: relative` 以承载绝对定位

### Requirement: 点击 CopyButton 复制文本并显示反馈

CopyButton 被点击时 SHALL 将 `text` prop 写入系统剪贴板，并以图标切换提供视觉反馈。

#### Scenario: 复制成功
- **WHEN** 用户点击 CopyButton
- **THEN** SHALL 调用 `navigator.clipboard.writeText(text)` 写入剪贴板
- **AND** 按钮图标 SHALL 从 Copy 图标切换为 Check 图标
- **AND** 2 秒后 SHALL 恢复为 Copy 图标

#### Scenario: 复制失败静默降级
- **WHEN** `navigator.clipboard.writeText` 抛出异常（权限拒绝或非 secure context）
- **THEN** SHALL 静默忽略，不显示错误提示
- **AND** 按钮状态 SHALL 保持不变（不切换为 Check）

### Requirement: OutputBlock 代码块提供 overlay copy

OutputBlock 组件（承载 AI 代码高亮输出）SHALL 在 hover 时右上角显示 overlay CopyButton。

#### Scenario: hover 出现 copy 按钮
- **WHEN** 用户将鼠标悬停在 OutputBlock 上
- **THEN** 右上角 SHALL 出现 CopyButton（overlay 模式）
- **AND** 点击后 SHALL 复制 OutputBlock 的完整 `code` 文本

#### Scenario: 离开 hover 隐藏
- **WHEN** 用户鼠标离开 OutputBlock
- **THEN** CopyButton SHALL 渐隐（transition opacity）


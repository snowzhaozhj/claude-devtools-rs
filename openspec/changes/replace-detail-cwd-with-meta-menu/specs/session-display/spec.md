## ADDED Requirements

### Requirement: SessionDetail 顶 bar meta-action menu 入口

SessionDetail 顶 bar SHALL 在 `.top-meta` 区（与既有 `[Context N]` toggle 并列）渲染一个 icon-only overflow menu trigger（下文统称 "meta-action menu"），承载会话级 on-demand 操作。trigger SHALL 复用 `.top-badge` 样式 token（`13px` icon、padding `6px 10px`、`border-radius 6px`），与 `[Context]` 共享视觉语言。

#### Scenario: meta-action trigger 渲染位置与形态

- **WHEN** SessionDetail 加载完成
- **THEN** 顶 bar 右侧 `.top-meta` 区 SHALL 渲染一个 icon-only `MoreHorizontal` (`⋯`) button
- **AND** trigger SHALL 位于 `[Context N]` toggle 的左侧（trigger 在左，Context 在右），二者间距对齐 `.top-meta` 既有 `gap: 8px`
- **AND** trigger SHALL NOT 渲染数字 / pill / text label
- **AND** trigger 默认态 icon 颜色 SHALL 为 `text-muted`，hover 态升至 `text` 主色

#### Scenario: 点击 trigger 展开 menu

- **WHEN** 用户点击 meta-action trigger
- **THEN** SHALL 在 trigger 下方右对齐位置展开 menu overlay（top = trigger.bottom + 4px）
- **AND** menu SHALL 不绘制指向 trigger 的箭头
- **AND** menu SHALL 按以下顺序列出 action items：
  1. `在 Finder 中打开`（macOS）/ `在文件管理器中打开`（其他平台）—— 仅 Tauri runtime 渲染
  2. `复制工作目录路径`
  3. `复制 Session ID`
- **AND** 第 (1)(2) 项与第 (3) 项之间 SHALL 渲染 1px `border-subtle` 分隔线（仅当第 (1) 项存在时）

#### Scenario: 平台分支 — HTTP server mode 隐藏文件管理器项

- **WHEN** 应用运行在 HTTP server mode（无 Tauri runtime，`isTauriRuntime() === false`）
- **THEN** menu SHALL NOT 渲染「在文件管理器中打开」项
- **AND** menu 仅包含「复制工作目录路径」与「复制 Session ID」两项
- **AND** SHALL NOT 渲染任何分隔线

#### Scenario: 平台分支 — Tauri 桌面 mode 完整渲染

- **WHEN** 应用运行在 Tauri runtime 内（`isTauriRuntime() === true`）
- **THEN** menu SHALL 渲染全部三项 action items 并含分隔线

#### Scenario: 「在文件管理器中打开」调 plugin

- **WHEN** 用户在 Tauri runtime 下点击「在文件管理器中打开」menu 项 AND `detail.metadata.cwd` 非空
- **THEN** SHALL 调用 `@tauri-apps/plugin-opener` 的 `openPath(detail.metadata.cwd)` API 打开系统文件管理器并定位到该路径
- **AND** menu overlay SHALL 立即关闭

#### Scenario: 「在文件管理器中打开」失败反馈

- **WHEN** `openPath` 调用 reject（路径不存在 / 权限拒绝 / plugin 内部错误）
- **THEN** trigger 区 SHALL 临时显示 `打开失败` 红字反馈（`color: danger`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态
- **AND** SHALL NOT 弹出 modal / dialog / global toast
- **AND** SHALL 在浏览器 console 或后端 `tracing` 留错误日志

#### Scenario: 「复制工作目录路径」成功

- **WHEN** 用户点击「复制工作目录路径」menu 项 AND `navigator.clipboard.writeText(detail.metadata.cwd)` resolve
- **THEN** menu overlay SHALL 立即关闭
- **AND** trigger 区 SHALL 临时显示 `已复制` 文字反馈（`color: text-secondary`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态
- **AND** SHALL NOT 弹出 toast

#### Scenario: 「复制工作目录路径」失败

- **WHEN** `navigator.clipboard.writeText` reject（多见于 HTTP non-secure context 或权限拒绝）
- **THEN** trigger 区 SHALL 临时显示 `复制失败` 红字反馈（`color: danger`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态

#### Scenario: 「复制 Session ID」

- **WHEN** 用户点击「复制 Session ID」menu 项
- **THEN** SHALL 调 `navigator.clipboard.writeText(detail.sessionId)` 复制完整 session id 字符串
- **AND** 反馈状态 SHALL 与「复制工作目录路径」一致（成功 `已复制` / 失败 `复制失败`，1500ms 自动恢复）

#### Scenario: cwd 缺失时降级

- **WHEN** `detail.metadata.cwd` 为 `undefined` / 空字符串（如老 session jsonl 不含 cwd 字段）
- **THEN** menu trigger SHALL 仍渲染并可点击展开
- **AND** 「在文件管理器中打开」与「复制工作目录路径」两项 SHALL 渲染为 disabled 态（不响应点击 / `text-muted` 色 / `cursor: not-allowed`）
- **AND** 「复制 Session ID」项 SHALL 保持可用
- **AND** menu SHALL NOT 渲染额外提示文案（disabled 态本身已传达）

#### Scenario: menu overlay 关闭行为

- **WHEN** menu 处于 open 态 AND 用户点击 menu 外区域 OR 按 `Esc` 键 OR 点击任一可用 menu 项
- **THEN** menu overlay SHALL 关闭
- **AND** trigger 焦点 SHALL 保持（键盘焦点回到 trigger）

#### Scenario: trigger 键盘可达性

- **WHEN** 用户使用键盘 Tab 移动焦点到 meta-action trigger
- **THEN** SHALL 渲染 `focus-visible` 蓝色 outline ring
- **AND** 按 `Enter` 或 `Space` SHALL 展开 menu
- **AND** menu open 态下方向键 SHALL 在 enabled menu 项之间移动焦点（disabled 项 SHALL 跳过）

#### Scenario: menu container ARIA 语义

- **WHEN** menu overlay 处于 open 态
- **THEN** menu 容器元素 SHALL 设 `role="menu"` 与 `aria-orientation="vertical"`
- **AND** trigger 元素 SHALL 设 `aria-haspopup="menu"` 与 `aria-expanded="true"`（关闭态切 `aria-expanded="false"`）
- **AND** trigger 元素 SHALL 设 `aria-controls=<menu-id>` 指向 menu 容器 id
- **AND** menu 中每个分组（cwd 操作组与 session id 操作组）SHALL 用 `role="separator"` 元素分隔（仅当多于一组时）

### Requirement: SessionDetail 顶 bar 不渲染完整 cwd 文本

SessionDetail 顶 bar SHALL NOT 在 `.top-stats` 行、`.top-titles` 区或任何常驻位置直接渲染完整 `cwd` 路径文本。完整 cwd 路径 SHALL 仅通过 meta-action menu 的 on-demand 操作（在文件管理器打开 / 复制路径）暴露。

#### Scenario: top-stats 行不含 CWD chip

- **WHEN** SessionDetail 加载完成
- **THEN** `.top-stats` 行 SHALL NOT 渲染 `CWD` label
- **AND** `.top-stats` 行 SHALL NOT 渲染任何完整 cwd 字符串
- **AND** `.top-stats` 行 SHALL 仅包含定长 / 短数字量化指标（AI / USER / TOOLS / TOK / LAST）

#### Scenario: top-stats 单行不触发 wrap

- **WHEN** SessionDetail 顶 bar 渲染于任意窗口宽度（≥ 最小桌面宽度 `800px`）
- **THEN** `.top-stats` 行 SHALL 单行渲染所有指标，不触发 `flex-wrap`
- **AND** `.top-stats` CSS SHALL 设 `flex-wrap: nowrap`
- **AND** SHALL NOT 在第一行末尾出现孤悬分隔符 `·`

#### Scenario: LAST 时间精度降级

- **WHEN** `.top-stats` 行渲染 LAST 时间
- **THEN** SHALL 显示分钟级 `HH:MM` 精度（如 `19:50`）
- **AND** SHALL NOT 显示秒级 `HH:MM:SS` 精度
- **AND** 时间格式与 sidebar 「刚刚 / 18m / 1h / HH:MM」时间显示密度对齐

## ADDED Requirements

### Requirement: Worktree chip 右键菜单

`WorktreeChipCluster.svelte` 渲染的每个 `.worktree-chip` 元素 SHALL 通过 `use:contextMenu` action 挂载右键菜单，让用户对 worktree 路径执行"复制路径 / 在编辑器打开 / 在终端打开 / 在 Finder/Explorer 中显示"等核心操作。菜单 items 由 `buildWorktreeChipItems` factory 构造；factory 入参含 `{ path: string; name: string }`；`open_in_terminal` / `open_in_editor` 走对应 IPC，路径长时通过 `pathLabel: { short, full }` 截断显示。

#### Scenario: 右键 worktree chip

- **WHEN** 用户在 sidebar 任一 `.worktree-chip` 上右键
- **THEN** SHALL 弹出含 "复制路径"、"在编辑器打开"、"在终端打开"、"在 Finder/Explorer 中显示" items 的菜单
- **AND** 触发位置遵循 `frontend-context-menu` viewport 边界 clamp 规则
- **AND** chip 上 `use:contextMenu` action 的 `stopPropagation` 阻止事件 bubble 到 sidebar 会话项的右键 handler

#### Scenario: 在终端打开 worktree 目录

- **WHEN** 用户点击 "在终端打开"
- **THEN** SHALL 调 `open_in_terminal(worktree.path)` IPC
- **AND** 后端按 Settings `terminalApp` 分流；macOS 走 `open -a <App> <path>`、Windows 走三级 fallback（`wt.exe` → PowerShell → cmd）、Linux 走 `x-terminal-emulator --working-directory=<path>` 或 DE-specific 命令
- **AND** 终端 app SHALL 仅 cd 到 worktree 目录，**不**执行任何 shell 命令

#### Scenario: 在编辑器打开 worktree

- **WHEN** 用户点击 "在编辑器打开"
- **THEN** SHALL 调 `open_in_editor(worktree.path, None, None)` IPC（不带行号参数）
- **AND** 后端按 Settings `externalEditor` 分流；以目录形式打开（VS Code/Cursor 直接接受目录参数；Zed/Sublime 同；System fallback 走 `open <path>` 等价行为）

### Requirement: 项目卡右键菜单

`Sidebar.svelte` 渲染的每个项目卡（含项目名称 + worktree chip cluster 的容器）SHALL 通过 `use:contextMenu` action 挂载右键菜单，items 由 `buildProjectCardItems` factory 构造，包含"复制项目路径 / 复制项目名 / 在编辑器打开项目 / 在终端打开项目根目录"。项目卡级菜单与 worktree chip 级菜单 SHALL 通过事件 `stopPropagation` 互不穿透——chip 级 action 拦截后，事件不冒泡到 project card；project card 级 action 仅在用户点中卡片本体（非 chip）时触发。

#### Scenario: 右键项目卡

- **WHEN** 用户在项目卡的非 chip 区域（项目名称 / 卡片背景）上右键
- **THEN** SHALL 弹出含 "复制项目路径"、"复制项目名"、"在编辑器打开"、"在终端打开" items 的菜单

#### Scenario: 项目卡 vs chip 菜单互不穿透

- **WHEN** 用户在项目卡内部的 worktree chip 上右键
- **THEN** chip 的 `use:contextMenu` action SHALL 优先触发并 `stopPropagation`
- **AND** 项目卡级菜单 SHALL **不**触发
- **AND** 用户感知：右键 chip 弹 chip 菜单；右键卡片其它区域弹项目卡菜单

#### Scenario: 在编辑器打开项目根目录

- **WHEN** 用户点击 "在编辑器打开"
- **THEN** SHALL 调 `open_in_editor(project.path, None, None)` IPC
- **AND** project.path 是已发现项目的根目录绝对路径

### Requirement: Sidebar 既有"右键菜单" Requirement 保持不变

Sidebar 会话项的右键菜单（已有 Requirement "右键菜单"）SHALL 不被 Phase 2 改动——会话项菜单 items（"在新标签页打开 / Open in New Pane / 置顶 / 隐藏 / 复制 Session ID / 复制恢复命令"）保持原行为；Phase 2 仅在会话项之外的 worktree chip 与项目卡上**新增**菜单挂载点，不影响会话项行为。

#### Scenario: 会话项右键菜单回归测试

- **WHEN** 用户在 sidebar 会话项上右键（既不是 worktree chip 也不是项目卡）
- **THEN** SHALL 弹出 Phase 1 既有会话项菜单（"在新标签页打开 / Open in New Pane / 置顶/取消置顶 / 隐藏/取消隐藏 / 复制 Session ID / 复制恢复命令"）
- **AND** 菜单内容、顺序、文案 SHALL 与 Phase 1 完全一致

## MODIFIED Requirements

### Requirement: Sidebar Memory 入口

Sidebar SHALL 在当前选中 group 存在 memory layers 时显示 `Memory (N)` 入口，其中 `N` 为可展示 memory layers 数量。点击入口 SHALL 调用 tab 系统打开该 group 的 Memory tab。若当前 group 没有 memory layers，Sidebar SHALL NOT 显示 Memory 入口。

**Anchor 选择**：sidebar 顶部 memory 入口的可见性条件与点击行为 SHALL 用 group 内"repo 根 / main worktree / 第一个 worktree" fallback 链选出的 worktree id 作为 query key（`getProjectMemory` IPC 入参与 `openMemoryTab` projectId 使用同一 anchor），**不**跟随 worktree filter 漂移。理由：memory 文件物理上写在 Claude Code 父进程 cwd 编码出的 project_dir 下（`~/.claude/projects/<encoded-cwd>/memory/`），绝大多数用户只在 repo 根目录跑 Claude Code，每个非 repo 根 worktree 各自的 encoded project_dir 下并不存在 memory 目录；若 anchor 跟随 worktree filter，切到具体 worktree 后查询返回 `count=0` 让入口消失，与用户"memory 是 repo 级别"的心智模型错位。

本约束与 D7（Requirement `selectedGroupId 与 worktree id 分层维护` 表 row 970）不矛盾——后者约束 query id 的形态（仍是一个 worktree id，与 detail API 保持同形），本约束在 query id 形态不变的前提下细化"哪个 worktree id"。

`pin/hide` 等其它 per-project state 仍走"跟随 worktree filter"的 anchor（per-worktree 隔离对置顶/隐藏有真实语义价值），不在本 Requirement scope 内。

#### Scenario: 当前 group 有 memory 时显示入口
- **WHEN** 当前选中 group 内 repo 根 worktree 的 memory discovery 返回 `hasMemory = true` 且 `count = 11`
- **THEN** Sidebar SHALL 在会话列表上方显示 `Memory (11)` 入口

#### Scenario: 点击 Memory 入口打开 group repo 根的 Memory tab
- **WHEN** 用户点击 Sidebar 的 `Memory (N)` 入口
- **THEN** 系统 SHALL 调用 tab 系统打开当前 group repo 根 worktree 的 Memory tab（即 `openMemoryTab(<group repo root worktree id>, "Memory")`）
- **AND** 即使当前 worktree filter 选了非 repo 根 worktree，打开的 tab projectId 仍 SHALL 为 repo 根 worktree id

#### Scenario: 当前 group 无 memory 时隐藏入口
- **WHEN** 当前选中 group 内 repo 根 worktree 的 memory discovery 返回 `hasMemory = false` 或 `count = 0`
- **THEN** Sidebar SHALL NOT 渲染 Memory 入口

#### Scenario: 切到非 repo root worktree 时 memory 入口仍显示 group 维度的 memory
- **WHEN** 用户在含 main + feat-x 双 worktree 的 group 内（main worktree memory `count = 3`、feat-x worktree memory `count = 0`），切 worktree filter 从"全部"到 feat-x
- **THEN** Sidebar 顶部 `Memory (3)` 入口 SHALL 仍可见且数量不变
- **AND** memory IPC query 入参 SHALL 仍是 main worktree 的 id（不是 feat-x 的 id）

#### Scenario: 切回"全部 worktree"后 memory 入口保持
- **WHEN** 用户在多 worktree group 内从 feat-x worktree filter 切回"全部"
- **THEN** Sidebar 顶部 memory 入口 SHALL 持续可见且数量与 repo 根 worktree memory count 一致

#### Scenario: 切换 group 刷新 Memory 入口
- **WHEN** 用户从 group A（repo 根 worktree 有 memory）切换到 group B（repo 根 worktree 无 memory）
- **THEN** Sidebar SHALL 隐藏 Memory 入口，并继续显示 group B 的会话列表

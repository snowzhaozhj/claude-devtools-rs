## Why

用户报告：切换到某些 worktree 之后，会话列表页顶部的 memory 组件会消失。

实测复现：在本仓库 130+ 个 worktree（`.claude/worktrees/*`）中，**仅** repo 根 cwd 编码出的 project_dir（`~/.claude/projects/-Users-...-claude-devtools-rs/memory/`）下有 12 份 memory `.md` 文件；所有 worktree 子 project_dir（`-Users-...-claude-devtools-rs--claude-worktrees-XXX/`）下**全部**没有 memory 目录。

根因（`ui/src/components/Sidebar.svelte`）：sidebar memory 入口的 anchor 跟随 `worktreeFilter`——切到具体 worktree 时 `loadProjectMemory(<worktree-id>)` 让后端按 `~/.claude/projects/<wt-id>/memory/` 查 → 物理上不存在 → `count=0` → 渲染守卫 `{#if memoryCount > 0}` 不通过 → 入口消失。

用户的实际心智模型是 "memory 是 repo 级别的"——同一个 repo 不论选哪个 worktree 都应看到自己写过的 memory；当前的 per-worktree anchor 与该心智模型错位。

D7 spec 表 row 970 写了"项目 memory / prefs（如有 per-project state） — worktree id — 不变（维持 per-worktree，与 detail API 一致）"——本约束讲的是 query ID 的形态（仍是 worktree id），但当前 sidebar 实现把 anchor 选择**也**绑到 worktree filter 上，是过度引申。Memory 文件物理上由 Claude Code 父进程 cwd 决定写到哪个 encoded project_dir，普通用户几乎只在 repo 根启动 Claude Code，per-worktree memory 几乎没有真实落点。

## What Changes

- **sidebar-navigation**: 修改 Requirement `Sidebar Memory 入口`，显式约束 sidebar 顶部 memory 入口的 anchor 选择规则——SHALL 用 group 内 repo 根 / main worktree fallback 链路恒定锚定，**不**跟随 worktree filter 漂移；query ID 仍为 worktree id（保持 D7 row 970 query id 一致性约束不变），但该 worktree id 恒为 group 内 repo 根那一个。同步更新既有 Scenario "切换项目刷新 Memory 入口" 为 group 维度语义；新增 Scenario 覆盖"切到非 repo root worktree 时入口仍显示"与"点击入口打开的是 repo 根 Memory tab"。

  pin/hide 仍走 `anchorWorktreeId`（per-worktree 隔离对置顶/隐藏有真实语义价值——worktree 间 session 列表不同），不在本 change scope。

## Impact

- Affected specs: `sidebar-navigation`
- Affected code:
  - `ui/src/components/Sidebar.svelte`
    - 新增 `memoryAnchorWorktreeId` derived（恒定 group 内 repo 根 / main / first fallback，不读 worktreeFilter）
    - `loadProjectMemory` 调用 site（effect 内）切到该 anchor
    - `loadProjectMemory` 内 race guard 三处（cache hit SWR 异步 / cache miss 异步 / catch reset null）切到该 anchor
    - memory entry button onclick `openMemoryTab` 切到该 anchor
- Affected tests:
  - 反转 `ui/tests/e2e/memory-viewer.spec.ts` 第一个 case（"无 memory 的 worktree 不显示 Sidebar Memory 入口"）的预期：切到 feat-x 后 memory 入口 SHALL **仍**显示（来自 repo 根的 count=3），改 case 名 + 断言；保留 line 17+ 的"空 Memory tab 展示空状态"逻辑（直接 openMemoryTab 走 wt-feat 那条不变，仍是 per-worktree memory tab 的合法入口）
  - 新增 `ui/tests/e2e/sidebar-memory-vs-worktree.spec.ts`（已落地，2 个 case：切到 feat-x 仍显示 / 切回全部仍显示）
  - 补 codex 反馈的 coverage gaps：点击 memory 入口打开 repo 根 tab（不是 wt 的）/ 切 group 时旧 count 不残留
- Affected backend: 无（不动 `crates/cdt-api/src/ipc/local.rs::project_memory_dir`，IPC 协议形态不变）

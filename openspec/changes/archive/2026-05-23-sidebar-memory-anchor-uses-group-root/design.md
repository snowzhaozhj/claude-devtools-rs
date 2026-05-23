# Design

## D1: anchor 选择策略——独立 derived，不复用 anchorWorktreeId

**问题**：sidebar 顶部 memory 入口当前用 `anchorWorktreeId`（跟随 worktreeFilter）做 query；切到具体 worktree 时该 anchor 指向那个 worktree 自己的 encoded id，但物理上几乎无 memory 落点。

**候选方案**：

- **A. 复用 anchorWorktreeId，把"全部 + 跟随 filter"逻辑改成"恒定 group 内 repo 根"**（in-place 改）
- **B. 新增独立 `memoryAnchorWorktreeId` derived，`anchorWorktreeId` 维持不变**
- **C. 后端 `project_memory_dir` 加 worktree 间 fallback**（`<wt>/memory/` 不存在时 fallback 到 group 内 repo 根的 memory）

**决策**：选 **B**。理由：

- A 会让 `anchorWorktreeId` 失去 per-worktree 语义，破坏现有 17 处 anchor 使用点（hidden / pinned 过滤 / hiddenCount / projectIdForSession 等）的 per-worktree 隔离——pin/hide 是真正的 per-worktree state（worktree 间 session 不同，pin 应隔离），不能一刀切。
- B 把"sidebar memory 入口的 anchor"语义独立出来，与"sidebar pin/hide / context menu projectId"语义分开，名字明确，影响半径小（5 处使用点全在 sidebar 内部）。
- C 改后端：（a）后端 `project_memory_dir` 不知道 group / worktree 关系，要传额外的 group repo root id；（b）改 IPC 契约；（c）跨 layer 改动复杂度远高于 UI 单点。

**实现**：

```svelte
const memoryAnchorWorktreeId = $derived.by(() => {
  if (groupWorktrees.length === 0) return selectedGroupId;
  return (
    groupWorktrees.find((w) => w.isRepoRoot)?.id
    ?? groupWorktrees.find((w) => w.isMainWorktree)?.id
    ?? groupWorktrees[0].id
  );
});
```

fallback 链 `isRepoRoot → isMainWorktree → groupWorktrees[0]` 与 `anchorWorktreeId` "全部"模式下 fallback 链一致；区别仅在不再读 `worktreeFilter`。

## D2: D7 spec row 970 与新约束的关系——query id 形态不变，anchor 选择更细化

D7 Requirement `selectedGroupId 与 worktree id 分层维护` 表 row 970：

> 项目 memory / prefs（如有 per-project state） | worktree id | 不变（维持 per-worktree，与 detail API 一致） | per-project state

**解读**：该约束讲的是 IPC 入参的 ID **形态**——memory 查询的 IPC 入参仍是 worktree id（不是 group id），与 detail API 一致；本质是 "用哪种 id 当 query key" 的约束。

**新约束（本 change 加在 `Sidebar Memory 入口` Requirement 内）**：sidebar 顶部 memory 入口选用哪个 worktree id 作为 query key——SHALL 恒定为 group 内 repo 根 / main worktree fallback。

**两条约束不矛盾**：本约束的 query key 仍是一个 worktree id（满足 D7 row 970 query id 形态约束），只是把 "哪个 worktree id" 从"跟随 worktree filter"细化为"恒定 group repo 根"。

D7 row 970 描述里 "维持 per-worktree" 在 `pin/hide` 维度仍然成立（pin/hide 仍走 `anchorWorktreeId` 跟 filter 漂）；本 change 不动 pin/hide。

## D3: 反转既有 e2e `memory-viewer.spec.ts:4` 的处理

既有测试：

```ts
test('无 memory 的 worktree 不显示 Sidebar Memory 入口', async ({ page }) => {
  ...
  await page.locator('.dd-popover .dd-opt-label', { hasText: 'feat-x' }).click()
  await expect(page.getByRole('button', { name: /Memory \(/ })).toHaveCount(0)
})
```

该 case 锁定了"切 worktree filter 到 feat-x → memory 入口隐藏"的旧行为。本 change 后该断言不再成立。

**处理**：改写而非删除。case 名改为"切到无 memory 的 worktree 时 sidebar memory 入口仍显示 group 维度的 memory"，断言反转为入口仍显示。理由：保留这个测试位是有价值的——它仍覆盖"切 worktree filter 时 memory 入口可见性"路径，只是预期值反转。直接删掉会让该路径回归 risk 增加。

`memory-viewer.spec.ts` 第二个 case（line 17+，"空 Memory tab 展示空状态"）走 `__cdtTest.openMemoryTab('mock-rich-rust-wt-feat')` 直接传 worktree id 到 tab 系统——这个路径不通过 sidebar 入口，仍然合法（用户从 sidebar 入口点击只会打开 repo 根 memory，但通过 tab 系统直接传入 worktree id 仍是合法的 backdoor），不在本 change scope 改。

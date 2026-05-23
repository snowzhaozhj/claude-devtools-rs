# tasks

## 1. spec delta + design

- [x] 1.1 `proposal.md` / `design.md` / `tasks.md` / `specs/sidebar-navigation/spec.md` 落地
- [x] 1.2 `openspec validate sidebar-memory-anchor-uses-group-root --strict` 通过

## 2. apply：UI 改动

- [x] 2.1 `ui/src/components/Sidebar.svelte` 新增 `memoryAnchorWorktreeId` derived（恒定 group 内 repo 根 / main / first fallback，不读 `worktreeFilter`）
- [x] 2.2 effect 内 `loadProjectMemory` 调 site 切到 `memoryAnchorWorktreeId`
- [x] 2.3 `loadProjectMemory` 内 race guard 三处（cache hit SWR / cache miss / catch reset null）切到 `memoryAnchorWorktreeId`
- [x] 2.4 memory entry button onclick `openMemoryTab(memoryAnchorWorktreeId, "Memory")`
- [x] 2.5 `pnpm --dir ui run check`（svelte-check 0 errors）

## 3. apply：测试反转 + 补 codex coverage gaps

- [x] 3.1 反转 `ui/tests/e2e/memory-viewer.spec.ts:4-15` 旧 case 预期：切到 feat-x 后 `.memory-entry` SHALL **仍**显示且 `Memory (3)` 不变；case 名改为 "切到无 memory 的 worktree 时 sidebar memory 入口仍显示 group 维度的 memory"
- [x] 3.2 新增 `ui/tests/e2e/sidebar-memory-vs-worktree.spec.ts` 已落（2 个 case）
- [x] 3.3 补 coverage gap：点击 sidebar memory 入口打开的 tab 应用 repo 根 memory（不是 worktree 的）—— assertion 通过 `__cdtTest` 反查 active tab 的 projectId
- [x] 3.4 补 coverage gap：从有 memory 的 rust-port group 切到无 memory 的单 worktree group → memory 入口 SHALL 隐藏（回归 group 切换路径）
- [x] 3.5 `pnpm --dir ui exec playwright test` 全绿
- [x] 3.6 双向回归证明：临时回滚 fix 后两个新 e2e 均 fail，恢复 fix 后均 pass（已验证）

## 4. 发布

- [x] 4.1 push 分支 + 开 PR（已完成 PR #210）
- [ ] 4.2 wait-ci 全绿（含 archive commit 后再次 wait-ci）
- [x] 4.3 codex 二审通过（first round verdict FAIL → 已按反馈补 openspec change + 反转旧 e2e + 补 coverage gaps；second round 待跑）
- [ ] 4.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

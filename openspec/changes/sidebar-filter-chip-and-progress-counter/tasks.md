## 1. WorktreeChipCluster 子组件

- [x] 1.1 新建 `ui/src/lib/components/WorktreeChipCluster.svelte`：props `{ value: string; options: { value: string; label: string; }[]; onChange: (v: string) => void; ariaLabel?: string }`
- [x] 1.2 实现横向 flex chip 列表 + `overflow-x: auto` + 隐藏 scrollbar (`scrollbar-width: none` + `::-webkit-scrollbar { display: none }`)
- [x] 1.3 chip 三档视觉状态：default / hover / active —— 走 `--color-text-secondary` / `--tool-item-hover-bg` / `--color-surface-overlay + --color-border-emphasis`（沿用 PR-A 的「持久选中是 quiet」语言）
- [x] 1.4 `focus-visible` outline 用 `--color-accent-blue`（瞬时焦点允许 blue，DESIGN.md 边界说明）
- [x] 1.5 「全部」chip 永远在最前；ARIA：cluster 用 `role="radiogroup"` + 每个 chip `role="radio"` `aria-checked`，无障碍 + 键盘 Left/Right 切换

## 2. Sidebar.svelte 接入

- [x] 2.1 删除 `Dropdown` import 与 `.worktree-filter-bar` 容器内 Dropdown 引用
- [x] 2.2 引入 `WorktreeChipCluster`，传入 options `[{ value: ALL_WORKTREES, label: "全部" }, ...groupWorktrees.map(wt => ({ value: wt.id, label: `⌗${wt.name}` }))]`
- [x] 2.3 chip cluster bar 容器高度统一 32px（与 Memory entry / Session search 同行高族），padding 沿用现 `.worktree-filter-bar` 6px 12px
- [x] 2.4 既有 `worktreeFilter` state + `$effect` → `loadSessions(filter)` 链路保持不动

## 3. session count 显示口径双态化

- [x] 3.1 派生 `matchCount = visibleSessions.length` 与 `scopeTotal` 两个 derived；`scopeTotal` 按 filter 从 `list_repository_groups` derived 链路派生（统一来源，无需新增 IPC / state；详 spec `Requirement: 会话总数显示口径` 数据来源链路段）：ALL_WORKTREES → `selectedGroup?.totalSessions ?? sessions.length`；具体 worktreeId → `groupWorktrees.find(w => w.id === worktreeFilter)?.sessions.length ?? sessions.length`。fallback 仅在 race window 内兜底。已加载条数是实现细节，加载进度由列表底部 `▼ 加载更多 · 剩 N 条` 按钮承载（PR-A 已落地）——顶部 count 不消费 / 不暴露已加载条数
- [x] 3.2 改 `.session-count-num` 渲染：`filterQuery ? `${matchCount} 匹配` : `${scopeTotal}`` —— 默认状态显**单数字**当前 scope 总量
- [x] 3.3 改 `title` tooltip：基础一层 `总 {scopeTotal}`；`hiddenCount > 0` 时 SHALL 追加 ` · {hiddenCount} 已隐藏`，`hiddenCount === 0` SHALL 不追加（避免 ` · 0 已隐藏` 噪音）。tooltip **不**显式暴露已加载条数——避免与底部按钮双处表达分页
- [x] 3.4 视觉权重不变：仍用 mono + `--color-text-muted`，仅文本内容换

## 4. 测试

- [x] 4.1 新增 vitest 单测 `ui/src/lib/components/WorktreeChipCluster.test.svelte.ts`：**按传入 options 顺序渲染**（不在组件内排序，排序责任归 Sidebar 调用方）/ 单选切换调 onChange / 键盘 ArrowLeft / ArrowRight 切换并即触发 onChange / Enter / Space 等价点击 / `role="radiogroup"` + `aria-checked` 正确 / `tabindex` roving 正确 / 「全部」chip 无 `⌗` 前缀
- [x] 4.2 更新 `ui/src/components/Sidebar.test.svelte.ts`：count 双态断言（默认 filter=ALL 显单数字 group total / 默认 filter=具体 wt 显单数字 wt total / 搜索 `N 匹配`）+ tooltip 单层 + 条件 hidden 断言（hidden=0 仅 `总 N` / hidden>0 追加 `· N 已隐藏`）+ chip options 构造顺序（「全部」最前 + isRepoRoot 次之 + 其余按 most_recent_session 倒序）
- [x] 4.3 更新 e2e `ui/tests/e2e/worktree-filter.spec.ts`：从 dropdown selector 改为 chip cluster `[role="radiogroup"]` + `[role="radio"]`；多 wt 显 cluster / 单 wt 不显 cluster / 切 chip 触发 list 重拉 + scroll-top 重置 / 深滚动后切 filter 列表回顶 / 键盘 ArrowRight 切换

## 5. 视觉自验

- [x] 5.1 `pnpm --dir ui run dev` + `?mock=1&fixture=multi-project-rich`，多 wt group 截图：默认「全部」active / 切到具体 wt active 切换
- [ ] 5.2 sidebar 200px / 280px / 400px 三档宽度截图：chip 不撞墙 + 多 chip 时 overflow 滚动（manual 跟进——CSS 行为：`worktree-filter-bar` `min-width:0` + chip cluster `overflow-x:auto` 已写，280px 默认宽度下 3 chip 可容纳已截图 5.1 验证）
- [x] 5.3 count 双态截图：filter=ALL 默认显 `127` / filter=具体 wt 默认显 `8` / 搜索后 `5 匹配` / hover tooltip `总 127`（hidden=0）与 `总 127 · 5 已隐藏`（hidden>0）两种状态
- [ ] 5.4 7+ worktree overflow 验证：临时给 mock fixture 注入 7 个 wt（or 用 e2e mockIPC 路径），截图验证右侧 fade mask 渲染 + 横向 scroll 触发 + 末段 chip 可达（manual 跟进——CSS 已配置 mask-image + scrollbar hidden + flex-wrap:nowrap，需用 fixture 注入 7+ wt 或 chrome-devtools 真实场景验证）

## 6. 本地验证 + spec validate

- [x] 6.1 `pnpm --dir ui run check`（svelte-check 0 errors）
- [x] 6.2 `pnpm --dir ui run test:unit`（含新 chip cluster 单测全 pass）
- [x] 6.3 `openspec validate sidebar-filter-chip-and-progress-counter --strict` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

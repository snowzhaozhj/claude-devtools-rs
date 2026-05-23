## Context

PR-A 完成 sidebar 视觉重排后，剩两块行为契约级改动留给本 PR-B：

1. `Sidebar.svelte:793` 引用 `Dropdown.svelte size="sm"` 渲染 worktree filter——dropdown 是 macOS WKWebView 风格的"打开 → 选 → 关闭"两步交互，与 sidebar 整体「Quiet Debugging Workbench」语言脱节；用户切 wt 看不到全 wt 名（折叠在 dropdown 内），多 wt group 时尤其费眼力。
2. `Sidebar.svelte:822` 仅渲染 `{visibleSessions.length}` 单数字（"5"），用户读不出含义；hover tooltip 才显 `可见 5 / 总 127`。`visibleSessions` 受 hide / search 影响，单数字脱离 hide/search 上下文表达不准。本 PR 改成「默认单数字 = 当前 scope 总量；搜索激活 = 命中数」双态，hover tooltip 暴露完整含义；分页进度由 PR-A 已落地的底部 `▼ 加载更多 · 剩 N 条` 按钮承担——用户不感知客户端分页内部状态，顶部 count 不必表达。

两块改动都涉及 spec MODIFIED Requirement，必须走 openspec 而非直接 PR。

## Goals / Non-Goals

**Goals:**
- 替换 worktree filter dropdown 为 chip cluster：所有 wt 一眼可见 + 一次点击切换 + 单选语义不变
- 改 session count 显示口径为双态（默认单数字 `{scopeTotal}` / 搜索 `match 匹配` / hover tooltip 单层 + 条件 hidden）—— 用户不感知客户端分页（PR-A 已在底部「▼ 加载更多 · 剩 N 条」承担分页信号）
- 沿用 PR-A 已落地的视觉语言（mono + muted + 暖中性 indicator）
- 修正 spec 与代码现状的脱节（line 664 spec vs `Sidebar.svelte:822` 单数字实现）

**Non-Goals:**
- 不改 IPC 字段 / 后端 / `list_group_sessions` cursor 构造逻辑（filter 切换仍走既有 buildFilterCursor）
- 不改 worktree filter 的 server-side `Exhausted` cursor 语义（这是 sidebar-navigation D6 已有规约，本 PR 仅改前端控件形态）
- 不改 `loadMoreSessions` 翻页逻辑、`sessionsNextCursor` 状态；顶部 count 不派生 / 不暴露已加载条数（分页进度由 PR-A 已落地的列表底部 `▼ 加载更多 · 剩 N 条` 按钮承担）
- 不引入多选 chip / Shift+click 高亮模式（YAGNI · 单选已够，与原 dropdown 单选语义一致）
- 不动 PR-A 已落地的 meta 行 / Memory entry / pin icon / date label / 加载更多按钮

## Decisions

### D1: 控件形态从 Dropdown 改为 chip cluster

**选择**：新建 `ui/src/lib/components/WorktreeChipCluster.svelte` 子组件。横向 flex 布局，每个 wt 一个 chip；"全部" chip 在最前默认选中；chip 数过多时容器横向滚动（`overflow-x: auto` + 隐藏 scrollbar）。

**对比方案**：
- (A · 推荐) 自建 chip cluster：完全控制视觉与交互，与 sidebar 语言对齐
- (B) 沿用 Dropdown 但视觉降级：仍要打开才能看到全 wt，没解决核心痛点
- (C) Tab strip：tab 风格隐含"切换页面"语义，与 filter「过滤数据」语义不一致；且 7+ wt 时 overflow 处理与 chip cluster 一样
- (D) Segmented control（iOS / SwiftUI 风格的连片 button group）：整段共用一个外框 + 等宽分割线，单选语义匹配；但**等宽**约束让长 wt 名（>10ch）整段被压缩或必须等宽截断；且与 sidebar quiet 调性冲突（segmented control 视觉权重接近 toggle button bar，对比 chip 的"轻标签"形态过重）；7+ wt 时整段宽度爆炸更难处理
- (E) Pill button group（每个 button 独立胶囊）：与 chip cluster 语义重叠，但 "pill" 通常隐含 action（点击触发动作）而非 filter（点击切换可视集合）；视觉上 pill 比 chip 高 2-3px、padding 大、边框更明显，与 sidebar 紧凑度不符

最终 chip cluster 优于 (D)/(E)：每个 chip 自适应宽度、视觉权重最轻、和 PR-A 已落地的 `.session-wt-label` 形态完全连贯。

**风险与对策**：
- chip 多到撑开容器 → 横向滚动 + 隐藏 scrollbar（`-webkit-overflow-scrolling: touch`），鼠标 wheel 在 chip cluster 内自动转横向；右侧 fade mask 提示 overflow（详 D-V2）
- 选中态视觉强度 → 沿用 PR #146 / `DESIGN.md::The Persistent Selection Is Quiet Rule`：暖中性 `--color-surface-overlay` 背景 + `border-emphasis` 边框，**不**用 Focus Blue / Indigo（已被 ongoing / 焦点占用）

### D2: Session count 显示口径双态（仅总量 + 搜索命中）

**选择**：

| 状态 | 显示 | 语义 |
|---|---|---|
| 默认（无搜索） | `{scopeTotal}` 单数字 例 `127` | 当前 scope 总量 |
| 搜索激活（filterQuery 非空） | `{matchCount} 匹配` 例 `5 匹配` | 搜索命中数 |
| hover tooltip（任何状态） | `总 127`（hidden=0）/ `总 127 · 5 已隐藏`（hidden>0） | 总量 + 条件 hidden |

**关键认识修正（用户反馈）**：用户**不感知客户端分页**。"30/127" 让用户问"为什么是 30 不是 127" → 引入认知负担解释分页机制，且 sidebar 已有底部 `▼ 加载更多 · 剩 N 条` 按钮 + `已显示全部 N 条` 端状态（PR-A 已落地）承担分页进度信号——顶部 count 再表达"已加载 / 总"等于双处冗余。类比：邮件客户端显 "1234 封"、文件管理器显 "127 项"、Linear 显 "127 issues" 都是单总量，分页是实现细节。

`matchCount = visibleSessions.length`（filterQuery + !isHidden 后剩余）。`scopeTotal` 来源依 filter 而定（详 D3）。

**对比方案**：
- (A · 推荐) 双态 / 单总量：用户读到就是总量，分页进度由底部按钮承载，认知最低
- (B) 三态 / `loaded/total` 分页进度（前一版方案）：技术上准确但暴露实现细节；用户不感知分页时是冗余
- (C) 一直显 `visible/total`：hide 时分子分母不同集合，用户困惑
- (D) 加进度条：视觉过重，与 sidebar 「调试工作台」克制语言冲突

**风险与对策**：
- 用户从「30/127」迁移到「127」可能短暂困惑"是不是少了什么信号"→ 底部 `▼ 加载更多 · 剩 N 条` + hover tooltip 一起兜底，且 PR 描述显式说明语义变化
- tooltip 主条款一开始写「三层完整信息」与 hidden=0 省略 scenario 自相矛盾（codex 二审 finding #2）→ 简化为「单层 + 条件 hidden」消除矛盾

### D3: Count scope 跟随 worktree filter（避免分子分母混合口径）

**问题**：filter 选具体 wt 时，`sessions` 是 server-side filter 后的（仅含该 wt），但若分母仍取 `selectedGroup.totalSessions`（group 全集），用户看到 `5/127` 会误读为"还有 122 条要翻"——分子分母不在同一 scope（codex 二审 finding #1）。

**选择**：scopeTotal 跟 filter scope 走，**统一从 `list_repository_groups` derived，不引入第二个本地 state**：
- filter=ALL_WORKTREES：`scopeTotal = selectedGroup?.totalSessions ?? sessions.length`（grouper 算的 group 跨 wt 真值，fallback 仅在 race window 兜底）
- filter=worktreeId：`scopeTotal = groupWorktrees.find(w => w.id === filter)?.sessions.length ?? sessions.length`（该 wt 内 sessions array 长度，fallback 同上）

数据来源链路：`list_repository_groups` IPC 返回 `RepositoryGroup.totalSessions` 与 `RepositoryGroup.worktrees[].sessions: string[]` 两字段——前端 `selectedGroup` / `groupWorktrees` derived 直接消费这两者，无需再监听 `listSessions` / `list_group_sessions` 翻页 IPC 的 `result.total`（在 ALL scope 下两者等价，统一走 derived 链路避免命名链路冗余）。

**对比方案**：
- (A · 推荐) scopeTotal 跟 filter 走：用户读到 `8` 或 `127` 是当前 scope 的总量，与 chip 选中状态语义一致；切 chip 时 scope 切换 + 数字突变在视觉上是同一动作的两端反馈
- (B) 始终显 group 全集 + 在搜索框边显 wt scope hint：双数字 + hint 元素增加视觉负担，违背紧凑工作台
- (C) filter 选具体 wt 时仍显 group 全集（如 `127`）：用户在该 wt scope 下看到与该 scope 无关的数字，与 chip 选中语义脱节

**风险与对策**：
- silent 刷新仅刷 group totalSessions，不刷 wt 级 sessions.length → silent 路径下 filter=具体 wt 的 scopeTotal 不更新；mitigation：silent 刷新本就重新拉 `list_repository_groups`（前端 SWR 缓存+revalidate 模式），wt.sessions 会一同刷新
- 边缘场景：filter 切换瞬间 sessions 已清空但 scopeTotal 还是旧 scope 的 → effect 链路顺序保证：worktreeFilter 变 → loadSessions 清 sessions → scopeTotal derived 自动跟新 filter 派生

### D-V1: Chip 视觉规范（沿用 PR-A 已固化的"暖中性持久选中"语言）

**选择**：

```css
.worktree-chip {
  height: 24px;
  padding: 3px 10px;
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--color-text-secondary);
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
}
.worktree-chip:hover { background: var(--tool-item-hover-bg); }
.worktree-chip-active {
  background: var(--color-surface-overlay);
  color: var(--color-text);
  border-color: var(--color-border-emphasis);
}
.worktree-chip:focus-visible {
  outline: 2px solid var(--color-accent-blue);  /* 瞬时焦点允许 blue（DESIGN.md `The Persistent Selection Is Quiet Rule` 边界说明）*/
  outline-offset: 1px;
}
```

**理由**：与 PR-A meta 行 `.session-wt-label` 同字体（mono）+ 同色族（muted-secondary），让"sidebar 顶部的 wt chip"和"行内的 wt label"是同一信号语言的两个尺度（chip = group 内总览 + 过滤入口；行内 label = 单 session 归属）。引用 `DESIGN.md::The Persistent Selection Is Quiet Rule` 精确名（codex 二审 finding #6 修正：之前简称遗漏 "The" 与 "Rule" 后缀）。

### D-V2: 横向滚动 + 右侧 fade mask 作为正式视觉合同

**选择**：chip cluster 容器 `overflow-x: auto` + `flex-wrap: nowrap`，scrollbar 隐藏（`scrollbar-width: none` + `::-webkit-scrollbar { display: none }`），**且容器右边缘渲染 fade mask**（`mask-image: linear-gradient(to right, black calc(100% - 16px), transparent)` 或等价 `::after` 16px 宽线性渐变叠层）。chip 数 ≤ 4 时不滚动（自然 fit + mask 在右侧不可见因为没溢出）；≥ 5 时用户看到右边缘渐变到透明，立即理解"右侧还有更多 chip"。

**对比方案**：
- (A · 推荐) 横向滚动 + fade mask：保持单行高度（32px sidebar Row 2），mask 作为不可点击但视觉强的 overflow indicator，符合 PRODUCT.md「快速定位」原则
- (B) 仅横向滚动无 mask：隐藏 scrollbar 在 5+ chip 场景下后段 chip 不可发现，违背 PRODUCT.md（codex 二审 finding #4）
- (C) 换行多行：sidebar 顶部高度浮动（多 wt 时占两三行），破坏「顶部 region 100px 固定」的整体结构
- (D) overflow ellipsis 折叠到 "..." 按钮：交互层级加深一档（"全部 / 显示更多 / 二级菜单"），与 dropdown 痛点回归

### D-V3: 键盘 / ARIA 与 scroll position 重置（codex 二审 finding #3 / #7）

**键盘 / ARIA**：cluster 用 `role="radiogroup"` + `aria-label`，每个 chip 用 `role="radio"` + `aria-checked`。roving tabindex 模式：当前选中 chip `tabindex="0"`，其他 `tabindex="-1"`。键盘 ArrowLeft / ArrowRight 切换并即时触发选中（与点击语义一致）；Enter / Space 在某 chip 上等价点击；不绕回（最末按 ArrowRight 停在末尾）。这套行为在 spec MODIFIED Requirement 中以 SHALL 固化，避免实现层做减法。

**scroll position 重置**：filter 切换时除清 sessions + 重 cursor 外，**还**要把 session-list 容器 `scrollTop` 重置为 0——避免用户在长列表底部切 chip 后新列表初始停在中段。spec MODIFIED Requirement 加 SHALL 明确，scenario 覆盖深滚动场景。

### D-V4: 默认状态可视性（"全部" chip 不靠选中态而是文字差异化）

**问题**：用户切到具体 wt 后，"全部" chip 在视觉上是"未选中"，但仍是默认状态——如果它和其他未选中 chip 完全一样，用户分不清"我可以一键回到全部"还是"它是普通选项"。

**选择**：「全部」chip 永远在最前 + label 用纯文字（无 `⌗` 前缀），与具体 wt chip 的 `⌗{name}` 视觉前缀拉开权重；选中"全部"时仍走 active 视觉（surface-overlay + border-emphasis），未选中时仍是普通 chip 形态——靠位置（最前）+ 前缀（无 `⌗`）+ 大众化标签（"全部"）三重表达「这是 reset 入口」。

## Visual Contract

### Surface Decision

入口选择：**sidebar 顶部 Row 2**（保留 PR-A 落地的三行 region 结构：Memory · Worktree filter · Session search · 100px 固定高度）。chip cluster 高度 32px = 与 Row 1 / Row 3 同行高族。**不**移到 TabBar / UnifiedTitleBar——TabBar 是 per-pane 的 tab 列表，UnifiedTitleBar 是项目切换 + 全局状态，worktree 过滤是「当前 group 内子维度」语义，归属 sidebar 范围。

链回 `PRODUCT.md`：「克制、可信、工程化。界面语气接近 IDE / 调试器 / Linear 式工作台」—— Linear 风格 chip cluster filter 是该参照系内已被验证的模式。

### Visual Layer

- 颜色：沿用 `DESIGN.md::The Status Owns the Color Rule` —— chip 作为「持久选中」信号，**不**新增彩色装饰；选中态用暖中性 surface 抬升
- 选中态：沿用 `DESIGN.md::The Persistent Selection Is Quiet Rule` —— filter 选中是「持久态」（用户切完后保留直到下次切换或切 group 重置），**不**用 Focus Blue / Indigo 表达；走 surface-overlay + border-emphasis 双通道
- 边框 / 阴影：沿用 `DESIGN.md::The Border Before Shadow Rule` —— 选中态用 1px border-emphasis 边框，**不**加 shadow / blur / glass
- 字体：沿用 `DESIGN.md::The Machine Information Rule` —— wt 名是 git worktree identifier，属于"机器信息"，用 `var(--font-mono)`；"全部" 是自然语言，但本 chip 整体保持 mono 字号一致简化（视觉对齐成本 < 字体切换的扫读成本）

### State Coverage

| 状态 | 视觉 | 实现位置 |
|---|---|---|
| default（未选中） | transparent bg + secondary text + transparent border | `.worktree-chip` 基类 |
| hover | `--tool-item-hover-bg` 背景 | `.worktree-chip:hover` |
| active（持久选中） | `surface-overlay` + `text` + `border-emphasis` | `.worktree-chip-active` |
| focus-visible（键盘焦点） | `accent-blue` 2px outline + 1px offset | `.worktree-chip:focus-visible`（瞬时焦点允许蓝） |
| disabled / loading | 不存在（chip 是同步状态切换） | — |
| overflow（chip 数 > 容器宽） | 横向滚动 + scrollbar 隐藏 | `.worktree-chip-cluster { overflow-x: auto }` |
| 切 group 重置 | 自动回到「全部」（既有 `worktreeFilter = ALL_WORKTREES` 语义） | sidebar-navigation D6 已固化 |

### DESIGN.md delta plan

本 change 不引入新 token，复用既有 `--color-surface-overlay` / `--color-border-emphasis` / `--color-text-secondary` / `--tool-item-hover-bg` / `--color-accent-blue` / `--font-mono`。

archive 前**不**跑 `/impeccable extract`——chip cluster 在本仓只此一处使用（worktree filter），未达到「值得沉淀进 DESIGN.md 的 reusable token / 组件」门槛。如果未来出现第二处 chip cluster 用例（例如 tags filter / status filter），届时再 extract。

## Risks / Trade-offs

- **Risk**: chip cluster 横向滚动在 Tauri WKWebView 与 Vite 浏览器 mock 表现可能不同（macOS 的 momentum scroll vs 浏览器普通滚动）→ **Mitigation**: e2e + 手动 `just dev` 双环境验证；`scrollbar-width: none` 跨平台兼容
- **Risk**: 7+ worktree 时 chip 全部隐藏在右侧 overflow，用户看不到「我有这个 wt」→ **Mitigation**: 已在 D-V2 把右侧 fade mask 升格为正式视觉合同（写入 spec MODIFIED Requirement），不依赖事后验收；tasks 5.4 加 7+ worktree fixture / Storybook 视觉验证（codex 二审 finding #4）
- **Risk**: silent 刷新拉新 totalSessions 时短暂 race（sessions 已新但 selectedGroup.totalSessions 还旧 / 反之）→ **Mitigation**: 两者都是 derived from 同一 `repositoryGroups` / `sessions` state，刷新走 `list_repository_groups` SWR revalidate 后 derived 自动一致；不引入 `Math.min` 等防御性 clamp（会掩盖真 race，应改为保证 `repositoryGroups` 与 `sessions` 更新顺序一致）。spec Requirement「会话总数显示口径」section 既定「loadMore 不改 selectedGroup.totalSessions」保证分母稳定
- **Risk**: 主 spec `Requirement: 会话总数显示口径` 现有 `{visibleSessions.length}/{totalSessions}` 描述与代码现状（单数字）和本 change 新口径（默认单数字 `{scopeTotal}` / 搜索 `{matchCount} 匹配`）三方不一致 → **Mitigation**: 本 change 用 MODIFIED Requirement 完整重写该段，archive 后 spec 与代码就一致
- **Trade-off**: chip cluster 比 dropdown 占更多水平空间——单 chip ~50-80px，5 个 chip ~300-400px，sidebar 宽 200px 时必须 scroll；但 sidebar 默认 280px 下 3-4 chip 可 fit，多 wt（≥5）是少数场景
- **Trade-off**: 横向滚动隐藏 scrollbar 让用户不知道"还有更多"，靠 mask 渐变 / wheel-horizontal 训练用户发现——这是 chip cluster 模式的固有取舍

## Migration Plan

无运行时迁移——纯 UI 控件替换，不改持久化字段 / IPC / 后端。

部署：单 PR merge 即上线。回滚：revert PR 即可。

## Open Questions

无。

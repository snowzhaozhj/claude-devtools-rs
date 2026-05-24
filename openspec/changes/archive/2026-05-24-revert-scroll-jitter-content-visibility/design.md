## Context

### 现状链路

`ui/src/routes/SessionDetail.svelte` 当前在对话流容器上挂 `.msg-row-contained` 类，由 PR #108（archive change `2026-05-16-session-detail-scroll-cpu-opt`）的 D1 引入。CSS 定义为：

```css
.msg-row-contained {
  content-visibility: auto;
  contain: layout style;
  contain-intrinsic-size: auto 220px;
}
:global(.msg-row-contained:has(.mermaid-block)) {
  content-visibility: visible;
  contain: none;
}
```

应用范围：UserChunk / SystemChunk / CompactChunk 外层；AIChunk 仅 `.ai-body` 子区域且 `ongoing=false` 时挂（D2b 决策回避 AI header popover 裁剪）。

### 现场测量数据（2026-05-24 长会话 34 chunk）

| 配置 | 10 秒滚动 `scrollHeight` 变化次数 | 总幅度 | 单次最大 | dh 分布 |
|---|---|---|---|---|
| 现状（启用 `.msg-row-contained`） | 11 次 | 5291 px | 4180 px | -197 / -139 / -174 / -108 / +4180 |
| 全局 `content-visibility: visible !important` inject | 0 次 | 0 px | 0 px | — |

dh 负值集中在 -108~-197 = `220 estimate − 真实高度`，与 `contain-intrinsic-size: auto 220px` 完全吻合。`+4180` 一次发生在滚到顶部时，约 19 个 chunk 同时 uncontain 真布局的累计差值。

### 触控板 vs 键盘对照

`scrollHeight` 0 变化状态下，触控板滚动 scroll 事件 531 次内反向 7 次、每次 dtop 1-5 px——属 macOS 触控板惯性末段物理特性，与代码无关；键盘方向键滚同段无任何感知抖动。证实 `content-visibility` 是唯一可修源头。

### 相邻历史

PR #250（`67aa480`）的 commit message 自陈："`contain-intrinsic-size: auto 220px` 估算与真高偏差通常 100-200 px，长 conversation 视口扫过的 chunks 数 × 偏差 = reveal 后剩余距离合计**几百到几千 px**"——已识别量级但只补了"按钮跳到底部"一条路径的 bottom pin 兜底，未补普通滚动路径。

## Goals / Non-Goals

**Goals:**

- 完全消除长会话滚动时的可测 `scrollHeight` 跳变（基线：长会话上 10 秒滚动 `scrollHeight` 变化 ≤ 2 次 / 总幅度 ≤ 50 px）
- 保留 PR #108 D3 决策（无语言代码块默认 plaintext / 关 `highlightAuto`）—— 这才是 CPU 大头
- 提供 spec 级反模式约束句，防止后续 PR 又以"性能优化"为由引入同类机制
- 改动单 PR 完成、不引入新依赖、不动 IPC / 后端

**Non-Goals:**

- 不重新做 virtual list（PR #108 当时就已 Non-Goal，本次同样维持）
- 不调整 lazy markdown 机制（已验证 lazyMarkdown 关闭 `LAZY_MARKDOWN_ENABLED=false` 后抖动数据形态与开启一致——不是抖动根因）
- 不补"动态测量 + ResizeObserver 缓存"等防御纵深方案——只做最小可逆改动，避免引入新的 trade-off 表面
- 不动 PR #250 的 bottom pin 状态机（解决另一类问题，与本次抖动无关）

## Decisions

### D1：删除 `.msg-row-contained` 整套 CSS + 4 处模板应用，不留兼容入口

**选：** 彻底删除 CSS 定义、`:global(...:has(.mermaid-block))` 豁免规则、模板里 4 处 `class:msg-row-contained` 应用、3 处与该机制相关的遗留注释。

**替代：**

- (a) 保留 class 名但 CSS 改成空 — 留死代码诱导后续 PR 又往里加东西，违反 CLAUDE.md "Avoid backwards-compatibility hacks like ... // removed comments for removed code"。
- (b) 改用 `contain-intrinsic-size: auto`（不带 fallback 数值）让浏览器全用 last-known-size — 首次进入视口仍走 fallback 0 引起更大跳变；最坏情况；浏览器对 `auto` 不带值实现差异不稳定。
- (c) 把 220 px 调到接近真实平均高度（如 80 px）— 真实高度方差太大（短消息 23 px / 长 AI chunk 数千 px），任何静态估算都有相似量级偏差。

**理由：** 现场数据已证明 `content-visibility` 是唯一可测的客观抖动源；任何"调参"路线都改不掉物理本质——估算占位 vs 真实高度必然 mismatch。彻底删除是唯一可逆且不引入新 trade-off 表面的方案。

### D2：spec 反模式句沉淀位置

**选：** 在 `session-display` capability 的"对话流容器渲染"相关 Requirement 段中追加一句 SHALL NOT，明确"性能优化路径上 SHALL NOT 在对话流容器上使用 `content-visibility` / `contain-intrinsic-size` 做高度估算占位机制"，并附简短理由（"会引起 `scrollHeight` 反复跳变 → 滚动抖动；历史教训见 archive change `2026-05-16-session-detail-scroll-cpu-opt`"）。

**替代：** 写进 `.claude/rules/perf.md` 的反模式清单——位置更偏 contributor doc，但 spec 一级是行为契约，更难被"PR 评估时跳过 rules"。

**理由：** spec 反模式句进入 `openspec validate --strict` 必读路径 + propose 阶段 spec-purity ratchet 会扫到，防回归强度高于 rules 散文件。同时保留 rules/perf.md 不动——它已有"反模式清单"段，未来若要扩可在该段引用 spec。

### D3：移除整段 `Requirement: SessionDetail 滚动路径渲染隔离`，不只删 Scenario

**选：** 该 Requirement 的存在前提就是"用 content-visibility 优化离屏"——单独删 Scenario 留 Purpose 段会产生"Requirement 标题在但无任何具体 Scenario 约束"的悬空状态，违反本仓 spec purity ratchet。整段移除最干净。

**替代：** 保留 Requirement 标题 + 改写 Purpose 段为"对话流不使用渲染隔离" — 但单纯否定式 Requirement 没有可验证的 Scenario，等同悬空。

**理由：** 该 Requirement 与 D1（无语言代码块高亮）是 PR #108 内两个独立决策，本次只回退 D1 不动 D3。spec 中两个 Requirement 互不依赖，删一保一无副作用。

## Risks / Trade-offs

- **滚动 CPU 可能回升** → 删 D1 后离屏 chunk 重新参与 layout / paint。Mitigation：merge 前在 30 project / 538 session corpus 的长会话（≥ 1k chunk）上手动用 Activity Monitor 抓 5 秒滚动样本，确认 `claude-devtools-tauri` 进程 < 15%；若超阈值，开 followup 走 ResizeObserver 测量 + 高度缓存路径（与本 PR 解耦）。当前推测 CPU 大头是 D3 接管的 `highlightAuto`，单独删 D1 影响有限但**待实测**。
- **测试覆盖损失** → 现 `SessionDetail.test.svelte.ts:183` 那条 case 验证的是"mermaid 区域豁免 contained"，删类后失去测试目标。Mitigation：直接删测试 case 而非改写——无需为"已删除的行为"维护回归测试。
- **被后续 PR 重新引入** → 同类机制看上去对降 CPU 很有吸引力。Mitigation：D2 的 spec 反模式句 + 在 rules/perf.md 的反模式清单交叉引用 + archive change 历史可查。
- **D3（无语言高亮）保留是否仍 OK** → 与本次抖动无关；spec 段独立，不会因删 D1 引发联动。无 mitigation 需求。

## Migration Plan

单 PR 直接落地，无 dataflow 迁移、无 IPC 兼容性问题。回滚：`git revert` 单 commit 即恢复 D1 行为。

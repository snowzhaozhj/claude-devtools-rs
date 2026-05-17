# Design — session-detail-header-bottom-divider

## 上下文

`app-chrome` capability 的 `Requirement: chrome 与下方区域的分隔线只一条 1 px` 由 unified-title-bar change（archive `2026-05-17-unified-title-bar`）引入，目的是防止三处双线视觉加粗：

1. chrome 底部 1 px ↔ sidebar/pane TabBar 顶部 border
2. active tab indicator ↔ TabBar 行底 1 px
3. **SessionDetail 顶部章节 ↔ TabBar 行底 1 px**（D8 决策）

第 3 项落地为 Scenario "SessionDetail 顶部不与 TabBar 行底 border 重叠"，但 THEN 子句把约束写成"`SessionDetail.svelte:1072` 处 `border-bottom: 1px solid var(--color-border)` SHALL 移除"——绑定了 file:line 与 CSS 属性。

用户当前反馈："会话详情头部和底部不易区分"，需要在 top-bar 与 conversation 之间加一条 1 px 分隔线。该分隔线位于 top-bar **下方**（top-bar 自身 ~65 px 高），与上方 TabBar 行底 border **不相邻**，物理上不构成 D8 想防的"叠线加粗"。

## D1：放宽 Scenario "SessionDetail 顶部不与 TabBar 行底 border 重叠"

**选项**：

- **A**：保持字面 SHALL，用 `box-shadow: inset 0 -1px 0 var(--color-border)` 代替 border-bottom 实现同样视觉。
- **B**：放宽 Scenario，禁止仍限于"与 TabBar 行底紧贴"的 border，明确允许 top-bar 下方 border-bottom。
- **C**：完全删除该 Scenario，回到 Requirement 第 89 行的语义级表述。

**选 B**。

**理由**：
- A 是"绕开 spec"——用 box-shadow 模拟 border 视觉，将来 reviewer 看到会困惑"为什么不用 border"，spec 字面的束缚没有真正解决。
- C 太激进——D8 的视觉约束确有价值（防 TabBar 行底叠线），不应完全废除 Scenario。
- B 是精准修订：保留 D8 的真实意图（防紧贴叠线），放开被字面误伤的合理用法（下方分隔）。

**风险**：未来如果有人在 SessionDetail 内插入新的顶部章节（如 banner / status bar）紧贴 TabBar 行底加 border，新 SHALL 措辞仍能拦下——因为禁止的是"与 TabBar 行底紧贴的 border"，不是某个具体 file:line。

## D2：装饰竖条 `.top-rail` 直接删除（无 spec）

`.top-rail`（左侧 3px 装饰条 + 绝对定位）在 unified-title-bar archive 里只在 design.md 顺带提了"4 px accent rail"作为视觉记号，未进 spec SHALL。用户反馈视觉冗余，直接删除即可，不需要 spec 改动。

## D3：实施细节固定不入 spec

新 Scenario 措辞仍只约束**语义**（"与 TabBar 行底紧贴的 border SHALL NOT 存在"），不绑定 file:line / 具体 CSS 属性。如此：

- 重构 SessionDetail.svelte 行号变化时 spec 不破坏
- 未来用 box-shadow 还是 border-bottom 实现下方分隔均可
- spec 与代码可独立演化

## D4：不动 `Requirement: chrome 与下方区域的分隔线只一条 1 px`

Requirement 第 89 行末句"Pane 内 content 区...最顶部章节 SHALL NOT 渲染与上方 TabBar 行底 border 紧贴的另一条 border"已经是语义级表述，正是 D8 的真实意图。**保留不动**——本 change 只 MODIFIED 其下属 Scenario，让 Scenario 与 Requirement 顶层语义一致。

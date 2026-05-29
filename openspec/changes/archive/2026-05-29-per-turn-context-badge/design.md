# Design: Per-turn Context Badge + Visible Context

## Summary

为 SessionDetail 每个 AI turn header 新增 "Context +N" 可点击 badge（增量视角），同时在 token popover 内新增 "Visible Context" 折叠段（累积视角）。后端暴露已有但被丢弃的 per-turn stats 数据。

## Decisions

### D1: IPC 暴露 per-turn context summary（字段名 `turnContextStats`）

后端 `process_session_context_with_phases` 已按 AI group 聚合 `ContextStats`（含 `new_injections`, `tokens_by_category`, `new_counts`），当前在 `inject_context_annotations` 中被丢弃。直接暴露精简 projection 到 `SessionDetail`，字段名 `turnContextStats`（非 `perTurnContext`，避免与已有 `contextInjections` / `injectionsByPhase` 混淆——codex 审查 #1）。

**Why:** 500 turn session × 精简 summary ~180B/turn ≈ 90KB，远低于 1MB 预算；前端 badge 渲染路径零计算。命名选 `Stats` 后缀明确其为 summary map 而非 injection 列表。

### D2: Token popover 统一改为 click 触发

现有 token 是 hover popover。新增 "Visible Context" 折叠段需要用户在 popover 内点击操作。Hover popover 鼠标移开即消失，无法操作内部交互元素。

**Why:** 统一两个 popover（badge + token）的交互模型；减少用户困惑；支持 Visible Context 的展开/收起操作。

### D3: 两个 popover 互斥

Badge popover 和 token popover 同一时刻只能打开一个。打开一个时关闭另一个。

**Why:** 它们展示同一区域的两个视角（增量 vs 累积），同时存在会造成视觉混乱。共享 `openPopoverId` state 即可。

### D4: Badge 空态规则（含 token 阈值）

- `new_count === 0` → 不渲染
- `new_count === 1 && 只有 thinking-text category && newTokens < 1000` → 不渲染
- 其余情况 → 渲染 badge

**Why:** 避免每行都冒 "Context +1"。但如果 thinking-text 本身很长（≥1k tokens），仍然显示——用户需要知道大 thinking block 在消耗 context（codex 审查 #4：纯 count 过滤会误伤高 token 单项）。1k 阈值 = 约 750 字的 thinking，低于此属正常短推理。

### D5: Category 排序按 token 降序

Popover 内 category 列表按本轮新增 token 数降序排列，最消耗 token 的排最前。

**Why:** 用户关心"什么吃了最多 token"，不是类别字母顺序。

### D6: 稀疏 map 只含 new_count > 0 的 turn

`turnContextStats` 字段只序列化有新 context 注入的 turn（`new_count > 0`），跳过空 turn。

**Why:** 减少 payload 大小（实测大多数 session 只有 60-80% turn 有非 thinking 注入）。

### D7: Summary 包含 cumulative tokens_by_category

`TurnContextStats` 同时包含 `newTokensByCategory`（本轮新增）和 `cumulativeTokensByCategory`（到该轮为止的累积分布）。前者用于 badge popover，后者用于 token popover 的 Visible Context 百分比。

**Why:** codex 审查 #2 指出只有 `tokensByCategory` 语义不清——badge 需要新增，Visible Context 需要累积。拆成两个字段消除歧义。`cumulativeEstimatedTokens` 也改名明确是累积值。

### D8: 前端 injection 按 turn memoize

前端一次性在 session 加载时把 `contextInjections` 按 `aiGroupId` 建 `Map<string, ContextInjection[]>` 缓存，badge popover 直接查 map 拿明细，避免每次 render O(turns × injections) 扫描。

**Why:** codex 审查 #6 指出 2000-turn session 反复 filter 是首先崩的性能点。一次性 memoize 在 session 加载时 O(N) 建 map，后续查询 O(1)。

### D9: 互斥 popover ID 含 chunkId 保证跨 turn 唯一

`popoverId` 格式为 `${chunkId}:context` / `${chunkId}:tokens`，保证不同 turn 的 badge 不互相冲突。

**Why:** codex 审查 #additional 指出如果 ID 只用局部名称（`token` / `context`），不同 turn 会互相干扰。

### D10: 三层 context 展示入口的职责边界

| 入口 | 回答的问题 | 时间维度 | 触发方式 |
|------|------------|----------|----------|
| Context Badge | "这一轮新增了什么？" | 单轮增量 | 行内 click pill |
| Token Visible Context | "到这一轮为止，window 被什么占着？" | 累积到当前轮 | token click → 折叠展开 |
| ContextPanel 侧面板 | "当前 phase 的完整 context inventory" | 全 phase | 顶部 toggle |

**Why:** codex 审查 #7 指出三者有语义重叠风险。明确边界 + 文案差异化（badge 用 "New this turn"，token 用 "Visible at this turn"，panel 用 "Phase context"）消除困惑。

### D11: A11y 选 button + aria-expanded，不做 focus trap

Badge popover 是轻量 metadata popover（非 modal），采用 `<button>` + `aria-expanded` + Esc/outside click dismiss。不做 focus trap（focus trap 适用于真 modal dialog）。

**Why:** codex 审查 #additional 指出 focus trap 对 metadata popover 过重。

### D-V1: Badge 位置选独立 pill（方案 A 对齐原版）

Badge 作为独立交互元素放在 AI header 右侧 metadata 区：spacer 之后、token 数之前。不与 token 合并。

**Why:** 全对齐原版产品决策；badge 与 token 的语义不同（"这轮新增了什么" vs "这轮总共用了多少"）；合并会损失快捷性。

## Visual Contract

### Surface Decision

入口选择：AI turn header 行内 pill，不选独立行/区块/浮窗。

理由：context 注入信息是 per-turn metadata（和 token 数、duration 同级）。引用 `DESIGN.md::The Machine Information Rule`——路径、token、duration 等机器信息在同一密度层呈现。增量 context 信息同源，不需要独立 surface。

### Visual Layer

- Badge: 引用 `DESIGN.md::The Status Owns the Color Rule`——中性 neutral pill，不染色
- Badge typography: 引用 `DESIGN.md::The Machine Information Rule`——11px mono
- Popover: 引用 `DESIGN.md::The Border Before Shadow Rule`——真浮层用 shadow 合规
- 折叠段: 引用 `DESIGN.md::The Tool Density Rule`——不用 fluid heading，font scale 紧凑

### State Coverage

| 组件 | 状态 | 实现 |
|------|------|------|
| ContextBadge | hidden | `new_count === 0` 或空态规则命中时不渲染 |
| ContextBadge | default | neutral pill，`--color-surface-raised` bg |
| ContextBadge | hover | `--color-surface-overlay` bg, `--color-border-emphasis` border |
| ContextBadge | active (popover open) | `--color-surface-overlay` bg, border-emphasis, popover 展示 |
| ContextBadge | focus-visible | `0 0 0 2px rgba(59,130,246,.15)` ring |
| Badge popover | default | category 列表 + token 数 |
| Badge popover | empty (不该出现) | badge 有空态规则保护 |
| Token popover | collapsed | 仅 token breakdown（5 行） |
| Token popover | Visible Context expanded | 展开折叠段显示 category 累积分布 |
| Token popover | no context data | 不显示 Visible Context 段 |

### DESIGN.md delta plan

本 change 引入的值得沉淀的设计模式：
- **Click popover pattern**（区别于 hover tooltip）：用于含可操作内容的 metadata popover
- **互斥 popover 管理**：同一行内多个 click popover 的状态协调模式

archive 前跑 `/impeccable extract` 评估是否提升到 DESIGN.md。

## Data flow

```
┌─ cdt-analyze::context ─────────────────────────────────────────────┐
│ process_session_context_with_phases()                               │
│   → SessionContextResult {                                         │
│       stats_map: HashMap<String, ContextStats>,  ← 每个 AI turn    │
│       phase_info: ContextPhaseInfo,                                │
│     }                                                              │
└────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌─ cdt-api::ipc::local.rs ──────────────────────────────────────────┐
│ inject_context_annotations() 新增：                                 │
│   对 stats_map 遍历，生成精简 turn_context_stats:                   │
│     HashMap<String, TurnContextStats>                              │
│   其中：                                                           │
│     TurnContextStats {                                             │
│       new_count: u32,                                              │
│       new_tokens: u64,                                             │
│       new_tokens_by_category: TokensByCategory,                    │
│       counts_by_category: CountsByCategory,                        │
│       cumulative_estimated_tokens: u64,  // 到该轮累积 context     │
│       cumulative_tokens_by_category: TokensByCategory,             │
│     }                                                              │
│   稀疏 map：只含 new_count > 0 的 turn                             │
│   Key MUST equal AIChunk.chunkId byte-for-byte                     │
└────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌─ SessionDetail IPC ───────────────────────────────────────────────┐
│ + turn_context_stats: HashMap<String, TurnContextStats>            │
│   (新增字段，camelCase 序列化为 turnContextStats)                   │
│                                                                    │
│ context_injections: Vec<ContextInjection>  (不变)                   │
│ injections_by_phase: BTreeMap<...>         (不变)                   │
│ phase_info: ContextPhaseInfo               (不变)                   │
└────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌─ Frontend ────────────────────────────────────────────────────────┐
│                                                                    │
│ contextExtractor.ts 新增：                                         │
│   TurnContextStats type                                            │
│   getPerTurnSummary(turnContextStats, chunkId) → Stats | null      │
│   buildInjectionsByTurnMap(contextInjections) → Map<id, Inj[]>     │
│   shouldShowBadge(stats) → boolean                                 │
│                                                                    │
│ ContextBadge.svelte (新组件)：                                     │
│   props: stats, injections, popoverId, openPopoverId, onToggle     │
│   渲染 pill → click → popover                                     │
│                                                                    │
│ SessionDetail.svelte：                                             │
│   AI header row 加入 ContextBadge                                  │
│   Token .ai-tokens 改 click 触发                                   │
│   互斥 popover state: openPopoverId                                │
│   Token popover 内新增 VisibleContextSection                       │
└────────────────────────────────────────────────────────────────────┘
```

## IPC contract change

### `SessionDetail` 新增字段

```typescript
interface SessionDetail {
  // ... existing fields ...
  turnContextStats: Record<string, TurnContextStats>;
}

interface TurnContextStats {
  newCount: number;
  newTokens: number;
  newTokensByCategory: TokensByCategory;
  countsByCategory: CountsByCategory;
  cumulativeEstimatedTokens: number;
  cumulativeTokensByCategory: TokensByCategory;
}
```

Key 为 `AIChunk.chunkId`（byte-for-byte 相等），前端直接按 `chunk.chunkId` 查询。

### 现有字段不变

- `contextInjections` — 前端一次性建 `Map<aiGroupId, ContextInjection[]>` memoize，badge popover 按 key 查询明细
- `injectionsByPhase` — 不变
- `phaseInfo` — 不变

## Component design

### ContextBadge.svelte

```
Props:
  stats: TurnContextStats | null
  injections: ContextInjection[]  (pre-filtered via memoized map)
  popoverId: string               (format: "${chunkId}:context")
  openPopoverId: string | null    (current open popover)
  onToggle: (id: string) => void  (toggle callback)

Render:
  if stats === null → nothing
  if !shouldShowBadge(stats) → nothing
  else → <button class="context-badge" aria-expanded={isOpen}> Context +{stats.newCount} </button>

A11y:
  role="button" (semantic <button>)
  aria-expanded={isOpen}
  aria-label="Context injected this turn: {stats.newCount} items, ~{formatTokens(stats.newTokens)} tokens"
  Dismiss: Esc / outside click / scroll
  No focus trap (lightweight metadata popover, not modal)

Popover content:
  Title: "New Context Injected This Turn"
  Categories (filtered count>0, sorted by tokens desc):
    › {categoryName} ({count})  ~{tokens} tokens
  Footer:
    Total new tokens  ~{stats.newTokens} tokens
```

### Token popover 改造

```
Trigger: click (替代 hover)
Dismiss: outside click / Esc / scroll / toggle

Content (unchanged top section):
  Total       {total}
  Input       {input}
  Output      {output}
  Cache create {cacheCreation}
  Cache read   {cacheRead}

Content (new section, collapsed by default):
  ── Visible Context (≈{pct}%) ── ▾
  [expanded]:
    ~{category}  ~{tokens}  {pct}%
    ... (sorted by cumulativeTokensByCategory desc)
    ───
    ⓘ Accumulated across session, estimated from content length
  Note: pct = cumulativeEstimatedTokens / apiReportedTotalTokens * 100
  Uses cumulativeTokensByCategory from TurnContextStats (D7)
```

## Test strategy

| Layer | What to test |
|-------|-------------|
| IPC contract (`cdt-api`) | `per_turn_context` field exists, sparse (empty turns absent), summary values match expectations |
| Vitest unit | `ContextBadge` renders/hides per empty state rules, popover toggle, mutual exclusion |
| Vitest unit | `contextExtractor.ts` new functions: `getPerTurnSummary`, `getTurnInjections` |
| Playwright e2e | Badge visible in AI header, click opens popover, click token opens token popover, mutual exclusion |

## Risks

1. **Payload 增长**：500 turn × ~220B（含 cumulative 字段）= ~110KB，可接受。2000+ turn session 约 440KB，仍在 1MB 预算内。
2. **Token popover click 改造破坏现有交互习惯**：用户可能习惯 hover 看 token。Mitigate: 保留 cursor:pointer + 加 ⓘ 图标暗示可点击。
3. **估算值标注**：Visible Context 数据全是 `charCount / 4` 启发式。需 `~` 前缀 + hint 避免误导。
4. **新旧字段语义一致性**（codex #additional）：`turnContextStats` 是 summary map，`contextInjections` 是完整 injection 列表。两者 new count 必须一致——IPC contract test 覆盖。
5. **前端 memoize 失效**：session 实时刷新时 `contextInjections` 会变。需在 derived state 里依赖 `contextInjections` 数组引用变化时重建 map。

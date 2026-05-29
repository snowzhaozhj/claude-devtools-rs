# Tasks: Per-turn Context Badge + Visible Context

## 1. 后端：新增 TurnContextStats 类型

- [x] 在 `cdt-core/src/context.rs` 新增 `TurnContextStats` struct
- [x] 字段：`new_count: u32`, `new_tokens: u64`, `new_tokens_by_category: TokensByCategory`, `counts_by_category: CountsByCategory`, `cumulative_estimated_tokens: u64`, `cumulative_tokens_by_category: TokensByCategory`
- [x] `#[serde(rename_all = "camelCase")]`

## 2. 后端：SessionDetail 新增 turn_context_stats 字段

- [x] `cdt-api/src/ipc/types.rs` 的 `SessionDetail` 新增 `turn_context_stats: HashMap<String, TurnContextStats>`
- [x] `#[serde(default)]` 保持向后兼容
- [x] 更新 `ContextAnnotations` 内部 helper struct

## 3. 后端：inject_context_annotations 暴露 stats_map

- [x] `cdt-api/src/ipc/local.rs` 的 `inject_context_annotations` 中遍历 `stats_map`
- [x] 对每个 entry 生成 `TurnContextStats`（从 `ContextStats` 投影 new + cumulative 两组数据）
- [x] 只包含 `new_counts` 总和 > 0 的 entry（稀疏 map）
- [x] Key MUST equal `AIChunk.chunkId` byte-for-byte
- [x] 返回到 `ContextAnnotations` 中

## 4. 后端：IPC contract test

- [x] `cdt-api/tests/ipc_contract.rs` 新增测试：`turn_context_stats` 字段存在
- [x] 验证稀疏性（空 turn 不在 map 中）
- [x] 验证 key 属于 AI chunkId 子集
- [x] 验证 newCount 与 contextInjections 按 group 分组后一致

## 5. 前端：contextExtractor.ts 新增类型和函数

- [x] 新增 `TurnContextStats` TypeScript interface（含 cumulative 字段）
- [x] 新增 `getPerTurnStats(turnContextStats, chunkId)` → `Stats | null`
- [x] 新增 `buildInjectionsByTurnMap(contextInjections)` → `Map<string, ContextInjection[]>` (一次性 memoize)
- [x] 新增 `shouldShowBadge(stats)` → boolean（空态规则含 1k token 阈值）
- [x] 新增 `formatCategory(category)` → display name

## 6. 前端：ContextBadge.svelte 组件

- [x] 新建 `ui/src/components/ContextBadge.svelte`
- [x] 渲染 pill badge "Context +{n}"
- [x] Click toggle popover（popoverId 格式 `${chunkId}:context`）
- [x] Popover 内容：title + category 列表（按 newTokensByCategory 降序）+ total
- [x] A11y: `<button>` + aria-expanded + aria-label
- [x] Popover dismiss: outside click / Esc / scroll（无 focus trap）

## 7. 前端：Token popover 改 click 触发 + Visible Context

- [x] `.ai-tokens` 从 CSS hover 改为 JS click toggle（popoverId 格式 `${chunkId}:tokens`）
- [x] 新增 "Visible Context (≈{pct}%)" 折叠段
- [x] 折叠段用 `cumulativeTokensByCategory` 显示累积分布（按 token 降序）
- [x] pct = cumulativeEstimatedTokens / apiReportedTotalTokens * 100
- [x] 底部 hint: "Accumulated across session, estimated from content length"
- [x] 估算值统一加 `~` 前缀
- [x] ⓘ 图标暗示可点击

## 8. 前端：互斥 popover + SessionDetail 集成

- [x] SessionDetail.svelte 新增 `openPopoverId` reactive state
- [x] AI header row 加入 ContextBadge 组件
- [x] 两个 popover 互斥逻辑（打开一个关闭另一个）
- [x] `buildInjectionsByTurnMap` 作为 derived state，依赖 contextInjections 引用变化重建
- [x] 深色主题验证分隔线和 popover 视觉

## 9. 测试

- [x] Vitest: ContextBadge renders / hides per empty state rules (含 1k 阈值边界)
- [x] Vitest: contextExtractor 新函数（shouldShowBadge / buildInjectionsByTurnMap）
- [x] Vitest: popover toggle / mutual exclusion
- [x] Playwright e2e: badge 可见 + click 打开 + 互斥

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

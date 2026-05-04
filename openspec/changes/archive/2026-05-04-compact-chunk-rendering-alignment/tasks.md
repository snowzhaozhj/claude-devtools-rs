## 1. cdt-core: CompactChunk 加可选派生字段

- [x] 1.1 `crates/cdt-core/src/chunk.rs::CompactChunk` 加两个字段：`#[serde(default, skip_serializing_if = "Option::is_none")] pub token_delta: Option<CompactionTokenDelta>` + `#[serde(default, skip_serializing_if = "Option::is_none")] pub phase_number: Option<u32>`（注意 import `CompactionTokenDelta` from `crate::context`）
- [x] 1.2 `crates/cdt-core/src/chunk.rs` 文件末尾的 `roundtrip(&Chunk::Compact(CompactChunk { .. }))` 测试：补 `token_delta: None, phase_number: None` 让构造点编译通过；再加一个 case `Some(CompactionTokenDelta { .. })` + `Some(2)` 验证 camelCase 序列化键名 + roundtrip 一致
- [x] 1.3 `cargo test -p cdt-core` 全过

## 2. workspace: 4 处 struct literal 构造点同步补字段

codex 二审已定位的 4 处 `CompactChunk { ... }` 构造点（用 struct literal，非 `..Default::default()`），SHALL 同一轮 Edit 全部补 `token_delta: None, phase_number: None`，避免单点 Edit 触发 PostToolUse clippy hook 反复阻塞：

- [x] 2.1 `crates/cdt-core/src/chunk.rs:466-472`（roundtrip 测试 `Chunk::Compact(CompactChunk { .. })`）
- [x] 2.2 `crates/cdt-analyze/src/chunk/builder.rs:169-175`（emit 入口 `out.push(Chunk::Compact(CompactChunk { .. }))`）
- [x] 2.3 `crates/cdt-analyze/tests/context_tracking.rs:64-70`（context_tracking 集成测试 fixture）
- [x] 2.4 `crates/cdt-api/tests/ipc_contract.rs:316-322`（既有 IPC contract case；本 change Task 4 会再加新 case，与此不重复）
- [x] 2.5 grep 兜底：`grep -rn "CompactChunk {" crates --include="*.rs"` 看是否还漏，若有按编译错信号补全
- [x] 2.6 `cargo test -p cdt-analyze` 全过；既有 `Emit CompactChunks at compaction boundaries` Scenario 测试不能 break

## 3. cdt-api: SessionDetail 组装层派生填充（D1c phaseNumber + D1d tokenDelta + D6 bool 参数）

- [x] 3.1 在 `cdt-api` SessionDetail 组装入口顶部加 `const COMPACT_DERIVED_ENABLED: bool = true;` 回滚开关常量
- [x] 3.2 实现派生函数 `apply_compact_derived(chunks: &mut [Chunk], enabled: bool)`（**signature 仅 chunks + bool 两参数**——D1d 删 `context_info`，派生完全独立于 `ContextPhaseInfo`）：
  - `enabled == false` → 直接 return，不写入任何字段
  - `enabled == true` → 两趟扫描（避免 mutable borrow 冲突）：
    - **Pass 1**：扫 chunks 找所有 `Chunk::Compact` 的 index，同时维护 `compact_counter: u32 = 1`，对每个 compact at index `i`：(a) 调 `find_last_ai_before(chunks, i)` + `find_first_ai_after(chunks, i)`；(b) 取它们的 last/first response total tokens；(c) `compact_counter += 1`；(d) 记录 `(i, computed_delta, compact_counter)` 到一个 vec
    - **Pass 2**：遍历记录的 vec，对每个 `(i, delta, phase)` 匹配 `chunks[i]` 的 `Chunk::Compact(c)`，写入 `c.token_delta = delta`、`c.phase_number = Some(phase)`
  - 在派生模块**内部**实现 4 个 helper（**不**跨 crate 调 cdt-analyze 内部 fn）：
    - `fn find_last_ai_before(chunks: &[Chunk], i: usize) -> Option<&AIChunk>` —— `chunks[..i].iter().rev().find_map(|c| if let Chunk::Ai(ai) = c { Some(ai) } else { None })`
    - `fn find_first_ai_after(chunks: &[Chunk], i: usize) -> Option<&AIChunk>` —— `chunks[i+1..].iter().find_map(|c| if let Chunk::Ai(ai) = c { Some(ai) } else { None })`
    - `fn ai_last_response_total_tokens(ai: &AIChunk) -> Option<u64>` —— 反向扫 `ai.responses.iter().rev()`，找第一个有 `usage` 的 response 算总和（`input + output + cache_read + cache_creation`，对齐 `cdt-analyze::context::session.rs:220-242`）
    - `fn ai_first_response_total_tokens(ai: &AIChunk) -> Option<u64>` —— 同上正向扫
- [x] 3.3 在 `get_session_detail` 真实组装路径调用 `apply_compact_derived(&mut chunks, COMPACT_DERIVED_ENABLED)`，**位置在 chunks 全部产出之后、各 OMIT 函数之前**（OMIT 不改 chunk 顺序也不改 responses[i].usage，前后顺序对算法无影响；放在 OMIT 之前更接近"chunks 落定 → 派生 metadata → 应用 OMIT 瘦身"的清晰流水线）
- [x] 3.4 HTTP detail 路径已走 `get_session_detail` 共享入口（见 `crates/cdt-api/src/http/routes.rs::detail` + `get_sessions_by_ids` 也委托 `get_session_detail`），无需额外注入；`list_sessions` / `list_sessions_sync` 返回 `SessionSummary` 无 chunks，**不**调用派生

## 4. cdt-api: IPC contract test 覆盖新字段

- [x] 4.1 `crates/cdt-api/tests/ipc_contract.rs` 加 case "tokenDelta + phaseNumber camelCase 序列化"：构造 `SessionDetail` 含一条 `CompactChunk { token_delta: Some(CompactionTokenDelta { pre_compaction_tokens: 30000, post_compaction_tokens: 5000, delta: -25000 }), phase_number: Some(3), .. }`，序列化后断言 JSON 字符串含 `"tokenDelta":{"preCompactionTokens":30000,"postCompactionTokens":5000,"delta":-25000}` 与 `"phaseNumber":3`
- [x] 4.2 加 case "None 时字段省略"：`CompactChunk { token_delta: None, phase_number: None, .. }` 序列化 JSON 不含 `tokenDelta` / `phaseNumber` key（验 `skip_serializing_if`）
- [x] 4.3 `cargo test -p cdt-api --test ipc_contract` 全过

## 5. cdt-api: 派生算法单测（每个 Scenario 一个测试）

派生函数所在位置加单测，覆盖 ipc-data-api spec.md 的 11 个 Scenario：

- [x] 5.1 `derive_token_delta_computed_from_neighboring_ai`：相邻 AI 都有 usage → 算出正确 delta
- [x] 5.2 `derive_token_delta_none_when_no_ai_before`：compact 之前无 AIChunk → tokenDelta None
- [x] 5.3 `derive_token_delta_none_when_no_ai_after`：compact 在 chunks 末尾无后续 AI → tokenDelta None
- [x] 5.4 `derive_token_delta_none_when_ai_lacks_usage`：相邻 AI 的 responses 全 usage=None → tokenDelta None
- [x] 5.5 `derive_consecutive_compacts_share_identical_token_delta`：**A→B→AI 连续 compact 共享同一 delta**（codex 三轮验证关键修复证据；构造 chunks `[AI(usage=30000), Compact("c-1"), Compact("c-2"), AI(usage=5000)]`，断言 c-1.tokenDelta == c-2.tokenDelta == Some({30000, 5000, -25000})。D1d 独立 findLastAiBefore/findFirstAiAfter 让两个 compact 各自独立命中同一对 AI，**不**因 cdt-analyze 内部 compact_group_id 覆盖问题让 c-1 拿到 None）
- [x] 5.6 `derive_phase_number_assigned_by_ordinal`：单 compact 在 chunks 中第 1 个出现 → phase 2
- [x] 5.7 `derive_consecutive_compacts_get_distinct_phase_numbers`：A→B→AI → c-1 phase 2、c-2 phase 3（D1c 验证）
- [x] 5.8 `derive_phase_number_stable_when_compact_at_end`：compact 在 chunks 末尾 → phaseNumber 仍正确
- [x] 5.9 `derive_compact_followed_only_by_user_and_system`：compact 后仅 User/System 无 AI → phaseNumber=Some(2)、tokenDelta=None
- [x] 5.10 `derive_disabled_returns_all_none`：`enabled=false` 时所有 CompactChunk 的 tokenDelta / phaseNumber 都保持 None（D6 验回滚开关可测）

## 6. ui: api.ts interface 同步

- [x] 6.1 `ui/src/lib/api.ts::CompactChunk` interface 加 `tokenDelta?: CompactionTokenDelta | null;` + `phaseNumber?: number | null;`
- [x] 6.2 同文件加 `export interface CompactionTokenDelta { preCompactionTokens: number; postCompactionTokens: number; delta: number; }`（对齐 Rust 端 camelCase 序列化）

## 7. ui: fixture 加示例数据

- [x] 7.1 `ui/src/lib/__fixtures__/multi-project-rich.ts::compactChunk` 加 `tokenDelta: { preCompactionTokens: 30000, postCompactionTokens: 5000, delta: -25000 }` + `phaseNumber: 2`
- [x] 7.2 `npm run check --prefix ui` 全过（验类型）

## 8. ui: SVG 图标加 LAYERS / CHEVRON_RIGHT

- [x] 8.1 `ui/src/lib/icons.ts` 加 `export const LAYERS = "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"` 与 `export const CHEVRON_RIGHT = "M9 18l6-6-6-6"`（lucide path d 字符串风格，对齐已有常量）

## 9. ui: SessionDetail.svelte Compact 分支重做

- [x] 9.1 替换当前 `chunk.kind === "compact"` 分支：用 `<button>` 折叠头 + `{#if isExpanded}` 展开内容容器
- [x] 9.2 折叠头结构：ChevronRight (旋转 90 表示展开) + Layers + "Compacted" + token delta（`{pre} → {post}` + 绿色 `({|delta|} freed)`）+ Phase N 徽章 + 时间 `h:mm:ss a`，全部 inline-flex
- [x] 9.3 折叠状态用 `let isExpanded = $state(false);` per-chunk 局部维护（不进 tabStore）
- [x] 9.4 展开容器：复用 `attachMarkdown(chunk.summaryText, "system")` lazyMarkdown 渲染，外层 `max-h-96` 滚动 + 左侧 `border-left: 2px solid var(--chat-ai-border)`
- [x] 9.5 amber 风格 CSS：`.compact-button` 背景 `var(--tool-call-bg)` + 边框 `var(--tool-call-border)` + 文字 `var(--tool-call-text)`；`.compact-token-delta` "freed" 文字 `#4ade80`；`.compact-phase-badge` 背景 `rgba(99, 102, 241, 0.15)` + 文字 `#818cf8`
- [x] 9.6 token delta 渲染逻辑：`tokenDelta` 为 `null` 时整个 token delta 段不渲染；`phaseNumber` 为 `null` 时 Phase 徽章不渲染（D1 fallback）
- [x] 9.7 删除旧 `.msg-row-system` + `.msg-system` + `.system-content` 中仅 compact 用的 CSS（保留 system 用的）
- [x] 9.8 用 `npm run dev --prefix ui` + 浏览器 fixture（`http://localhost:5173/?mock=1&fixture=multi-project-rich`）目测 compact 折叠 / 展开 / token delta / phase 徽章渲染正确

## 10. ui: SessionDetail.svelte System 分支气泡对齐

- [x] 10.1 `chunk.kind === "system"` 分支：保留既有 `.system-header`（Terminal + System + 时间），把 `<pre class="system-pre">` 包进新的 `<div class="system-bubble">` 容器
- [x] 10.2 `.msg-row-system-left` 改 `display: flex; justify-content: flex-start`；内层 `.system-bubble-wrapper` 设 `max-width: 85%`
- [x] 10.3 `.system-bubble`: `background: var(--chat-system-bg)` + `border-radius: 16px 16px 16px 4px`（对齐 Tailwind `rounded-2xl rounded-bl-sm`）+ `padding: 12px 16px`
- [x] 10.4 `.system-pre` 内文字色改 `var(--chat-system-text)`，保留 `white-space: pre-wrap; font-family: var(--font-mono); font-size: 13px;`
- [x] 10.5 `npm run check --prefix ui` 通过；浏览器 fixture 目测 system 气泡视觉对齐原版（左对齐 + 圆角气泡）

## 11. ui: Sidebar.session-meta CSS bug 修复

- [x] 11.1 `ui/src/components/Sidebar.svelte` 的 `.session-msg-count` 加 `flex-shrink: 0; white-space: nowrap;`
- [x] 11.2 `.session-time` 加 `flex-shrink: 0; white-space: nowrap;`
- [x] 11.3 `.session-branch` 加 `min-width: 0; flex-shrink: 1;`（已有 `.session-branch-name` 的 ellipsis 配合生效）
- [x] 11.4 浏览器 fixture（`?fixture=multi-project-rich`）+ 拖拽 sidebar 至 220px 窄宽，目测"刚刚 / 1m / 1h" 等短时间不再竖排，分支名长时 ellipsis

## 12. 验证 / preflight

- [x] 12.1 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 12.2 `cargo fmt --all`
- [x] 12.3 `cargo test --workspace` 全过
- [x] 12.4 `npm run check --prefix ui` 全过
- [x] 12.5 `openspec validate compact-chunk-rendering-alignment --strict` 通过
- [x] 12.6 `cargo tauri dev` 真窗口手动验证：开一个含 compaction 的真实 session（如本会话），看 Compact 折叠头 / 展开 / token delta / phase；切到 system message 占据多的会话，看 System 气泡；切到 sidebar 窄宽，看 meta 行不竖排

## 13. codex 二审（行为契约改动 SHALL 跑）

- [x] 13.1 push 第一轮 commit 后调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 二审，prompt 列具体怀疑点（D1 派生算法边界、`apply_compact_derived` 调用时机是否在 OMIT 之前 / 之后、IPC contract test 是否真覆盖派生路径）
- [x] 13.2 codex 找到 bug：全部修完 + 单测覆盖，再调第二轮 codex 验证修法是否真解决
- [x] 13.3 codex 验证通过的 commit 才作为 PR 最后一个 commit；验证有 race / 边界仍未修干净 SHALL 不 push 到 PR

## Context

`crates/cdt-analyze/src/chunk/builder.rs` 已在 PR #114 / change `stable-chunk-ids` 给 `AIChunk` 引入 `ai_chunk_ordinals: HashMap<String, usize>`，按 `responses[0].uuid` 作为 base、出现次数作为 ordinal，生成形如 `ai:<base>:<n>` 的稳定唯一 `chunk_id`。同一改动**未**覆盖 `UserChunk` / `SystemChunk` / `CompactChunk`——这三类目前直接 `chunk_id: msg.uuid.clone()`。

`ipc-data-api` spec `Requirement: Stable chunk identifiers in SessionDetail` 同时要求"所有 `chunkId` MUST 唯一"和"非 AI chunk `chunkId` SHALL 等于自身 uuid"，在 JSONL 真实出现同 uuid 重复时（典型场景：`claude --bg` 启动 bg session 时把初始 prompt 以同 uuid 回放写入主 session JSONL，line 6 vs line 1077 一对真实命中），两条规则不可兼得，前端 `{#each detail.chunks (chunk.chunkId)}` 抛 `keyed each block has duplicate key` 详情页崩溃。

约束：
- 不破坏既有 chunk_id 形态——`expandedItems` / 搜索锚点 / 测试断言都基于裸 uuid，**首次出现**的 chunk_id MUST 保持 `== uuid`。
- 与 `AIChunk` 已有的 ordinal 模式概念对齐——只是 user/system/compact 三类共享同一计数器（uuid 不区分 kind）。
- 仅一段 IPC payload 字段的稳定性策略调整，不引入新 capability、不改字段名。

## Goals / Non-Goals

**Goals:**

- 同一 sessionId JSONL 出现重复 user / system / compact `uuid` 时，后端 `chunk_id` 集合保持唯一，前端不再抛 duplicate key。
- 首次出现的 chunk_id 与现状字节级一致——已部署/已截图的会话渲染不变。
- spec 文字与实现行为一致，消除"MUST 唯一" vs "SHALL 等于 uuid"的自相矛盾。

**Non-Goals:**

- 不改 `AIChunk` 的 `ai:<base>:<n>` 形态——已稳定且独立命名空间。
- 不查 JSONL 上游为什么会产生重复 uuid——那是 Claude Code 本身的写入策略，本端口只在下游 robust。
- 不引入 chunk_id 跨 session 唯一性——`chunkId` 只在单次 `get_session_detail` 返回内 unique 即可。

## Decisions

### D1: helper `next_non_ai_chunk_id(uuid, ordinals) -> String`

候选方案：

- **A. 全局共享一个 `chunk_id_ordinals: HashMap<String, usize>`（含 AI）**：所有 chunk 都走同一计数器。优点：统一；缺点：要重做 `AIChunk` 现有 `ai:<base>:<n>` 形态（兼容性破坏），且 AI 的 base 已带 `ai:` 前缀本身就不会与裸 uuid 撞，没有真撞车风险。
- **B. user/system/compact 三类共用一个 `non_ai_chunk_ordinals: HashMap<String, usize>`，AI 沿用 `ai_chunk_ordinals`（已存在）**：✓ 选这条。**理由**：(1) AI 已带 `ai:` 命名空间隔离，与 user/system/compact 的裸 uuid 永远不会跨 kind 撞；(2) 三类非 AI 的 uuid 都直接来自 JSONL `msg.uuid`，是同一命名空间，应共享计数器（防止 user uuid 与 system uuid 撞——理论上不应发生，但 robust 兜底零成本）；(3) 改动最小，只新增一个 HashMap + 一个 helper。
- **C. 每个 kind 独立 ordinals map**：三个 HashMap。优点：分类清晰；缺点：浪费——三类共享同一 uuid 命名空间，分三个 map 没语义价值，跨 kind 撞车反而漏检。

决策：**B**。新增 `non_ai_chunk_ordinals: HashMap<String, usize>`，与 `ai_chunk_ordinals` 平级穿在 build pipeline 中。

### D2: 后缀形态 `<uuid>` / `<uuid>:1` / `<uuid>:2`

候选方案：

- **A. `<uuid>` (count=0) → `<uuid>:1` (count=1) → `<uuid>:2` ...**：✓ 选这条。**理由**：(1) 首次出现保持裸 uuid，与现状字节级一致——前端 `expandedItems` 等已缓存的 chunk_id 不失效；(2) `:` 分隔与 AIChunk 的 `ai:<base>:<n>` 风格统一；(3) 重复出现的 chunk 本就因 uuid 重复无法稳定渲染，给它换 key 不破坏任何有效缓存。
- **B. 总是带 ordinal：`<uuid>:0` / `<uuid>:1`**：与 AI 完全对齐。缺点：所有现存会话的 chunk_id 都变形态，前端 `expandedItems` 全部失效一次，相当于 PR #114 当时刻意避免的"breaking all existing UI state"；测试断言全部要改。
- **C. `<uuid>` → `<uuid>#1` / `<uuid>#2`**：换 `#` 分隔。中性。

决策：**A**。`if count == 0 { uuid.to_owned() } else { format!("{uuid}:{count}") }`。

### D3: ordinal counter 何时增长 / 何时持久化

候选方案：

- **A. 在每次 `out.push(Chunk::User/System/Compact { chunk_id, ... })` 前调 helper，helper 内部 `*count += 1`**：✓ 选这条。**理由**：(1) 与 AIChunk `next_ai_chunk_id` 调用时机一致；(2) `tool_result only` 等被过滤掉的 user 消息**不**调 helper，不消耗 ordinal——保证 ordinal 反映"真正成 chunk 的次数"，session 未变化时多次调用 chunk_id 集合稳定（满足"未变化 session 重复调用时 chunkId 稳定"的 Scenario）。
- **B. 不论是否产 chunk 都计数**：会让 tool_result-only user 消息也消耗 ordinal，与 AI 行为不一致，且 spec 已强调"未变化 session 重复调用 stable"——本方案多余。

决策：**A**。

### D4: spec delta 写法

候选方案：

- **A. MODIFIED 整段 Requirement（含所有现有 Scenario）+ 新增一条 "重复 user uuid" Scenario**：✓ 选这条。**理由**：(1) Requirement body 描述本身需要改（"SHALL 等于自身 uuid" → "首次 SHALL 等于 uuid，重复时通过 ordinal 后缀消歧"）；(2) "非 AI chunk 使用自身 uuid" Scenario 标题与现状不一致，改为 "非 AI chunk 首次出现使用自身 uuid"；(3) 新增专门 Scenario 描述 bg 回放场景的 duplicate user uuid 行为，让 reviewer 一眼看到本 change 的真实意图。
- **B. 只新增 Scenario，不改 Requirement body**：会留下 Requirement body 与 Scenario 矛盾的痕迹。

决策：**A**。

## Risks / Trade-offs

- **[Risk] 前端缓存的 chunk_id（`expandedItems` 等）失效**：仅对"重复 uuid 的 chunk"——这些本就因 duplicate key 错误无法稳定渲染，无有效缓存可保。首次出现的 chunk 字节级不变。✓ 可接受。
- **[Risk] 测试覆盖不够**：`crates/cdt-analyze/src/chunk/builder.rs` 既有 `duplicate_assistant_response_uuid_gets_stable_unique_chunk_ids` 测试覆盖 AI，需镜像加一个 user 重复 uuid 的测试。→ 见 tasks.md。
- **[Risk] AI chunk_id 与 user 裸 uuid 同 namespace 撞**：AI 的 chunk_id 总是 `ai:<base>:<n>` 形态（带 `ai:` 前缀），与裸 uuid 永远不会撞。零风险。
- **[Trade-off] 不消除 JSONL 上游的重复 uuid 写入**：那是 Claude Code 本身的策略，端口只能下游 robust。✓ spec 与代码一致即可。

## Migration Plan

无运行时迁移——只增加一个内存 HashMap。已有 chunk_id 首次出现形态不变。

回滚策略：单 PR 内可 revert。

## Why

`get_session_detail` 的 spec（`ipc-data-api`）已要求"同一次返回内所有 `chunkId` MUST 唯一"，但同一段又规定 `UserChunk` / `SystemChunk` / `CompactChunk` 的 `chunkId` **SHALL 等于自身消息 `uuid`**——两条规则在同一 sessionId JSONL 里出现重复 `uuid` 时不可兼得。

真实数据已踩坑：Claude Code `claude --bg` 在主 session 启动 bg session 时，会把初始 prompt 以**相同 uuid** 回放到主 session JSONL（line 6 是用户原始输入、line 1077 是 bg session 启动回放）。结果前端 `{#each detail.chunks as chunk, i (chunkKey(chunk))}` 拿到两个相同的 `chunkId` → Svelte 抛 `keyed each block has duplicate key` 详情页崩溃。

PR #114 已经给 `AIChunk` 加了 occurrence ordinal 后缀消歧，但当时遗漏了 user/system/compact 三类——本 change 把同一去重策略对齐到这三类。

## What Changes

- **`ipc-data-api` spec**：放宽 `UserChunk` / `SystemChunk` / `CompactChunk` `chunkId` 规则——基底仍为自身消息 `uuid`，但 MUST 在同一次返回内出现重复 `uuid` 时通过 occurrence ordinal 等稳定后缀消歧，使整体 `chunkId` 集合 MUST 唯一。
- **`chunk-building` 实现（`crates/cdt-analyze/src/chunk/builder.rs`）**：新增共享 `non_ai_chunk_ordinals: HashMap<String, usize>`，user / system / compact 三处构造 chunk 时统一走 helper `next_non_ai_chunk_id(uuid, ordinals)`——首次出现保持 `chunkId == uuid`，重复出现追加 `:1` / `:2` ... 后缀。
- 加单元测试 `crates/cdt-analyze/src/chunk/builder.rs::duplicate_user_uuid_gets_stable_unique_chunk_ids`：fixture 含两条同 uuid 的 user 消息（模拟 bg 回放场景），断言 chunk 数为 2、两个 `chunk_id` 不同且都以共享 uuid 开头。
- 不破坏既有 spec scenario "非 AI chunk 使用自身 uuid"——首次出现仍 `chunk_id == uuid`，与现状一致；只有出现重复时才追加后缀。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`：修改 `Requirement: Stable chunk identifiers in SessionDetail` 与对应 Scenario，明确 user/system/compact 在重复 uuid 时的消歧策略。

## Impact

- 代码：`crates/cdt-analyze/src/chunk/builder.rs`（加 helper + 3 处 callsite + 1 测试）。
- 行为契约：`openspec/specs/ipc-data-api/spec.md`（一段 Requirement 描述 + 1 Scenario 补充）。
- 前端：无需改动——`chunkKey(c)` 仍直接用 `c.chunkId`，后端保证唯一即可。
- 兼容性：首次出现的 chunk_id 保持 `== uuid`，前端 `expandedItems` / 搜索锚点对未撞 uuid 的会话完全无感；只有发生重复时的"重复 chunk" 才换 key（这些 chunk 之前根本无法稳定渲染，本就不存在有效缓存状态）。

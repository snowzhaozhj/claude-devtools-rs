## 1. cdt-core：SemanticStep 新增 UserMessage variant

- [x] 1.1 在 `cdt-core/src/message.rs` 的 `SemanticStep` enum 新增 `UserMessage { uuid: String, text: String, timestamp: DateTime<Utc> }` variant，serde tag 为 `"user_message"`
- [x] 1.2 在 `ParsedMessage` struct 新增 `is_queued_input: bool` 字段，`#[serde(default, skip_serializing)]`

## 2. cdt-parse：识别 queued_command attachment

- [x] 2.1 在 `parser.rs::parse_entry_at` 中，`parse_message_type` 返回 None 后，追加 attachment/queued_command 检查逻辑：解析 `RawEntry` 新增 `attachment` 字段（`Option<RawAttachment>`），提取 prompt 构造 ParsedMessage
- [x] 2.2 新增测试：attachment queued_command 解析为 User 消息 + is_queued_input=true
- [x] 2.3 新增测试：attachment 非 queued_command（skill_listing）跳过
- [x] 2.4 新增测试：attachment queued_command prompt 为空时跳过

## 3. cdt-analyze：chunk-building inline embed

- [x] 3.1 在 `ChunkBuildState` 新增 `pending_user_messages: Vec<PendingUserMessage>` 字段（uuid + text + timestamp）
- [x] 3.2 `handle_user` 中 `is_queued_input` 消息不 flush、不产 UserChunk，push 到 pending
- [x] 3.3 `flush_with_responses` 中把 pending_user_messages 按 timestamp 插入 semantic_steps 精确位置后清空
- [x] 3.4 `drain_trailing_user_messages`：末尾 flush 后仍有 pending 的追加到最后 AIChunk；无 AIChunk 丢弃
- [x] 3.5 新增测试：queued input 不产 UserChunk、不打断 turn
- [x] 3.6 新增测试：UserMessage step 出现在正确时序位
- [x] 3.7 新增测试：连续多条 queued input 各自独立 step
- [x] 3.8 新增测试：trailing queued input 追加到最后 AIChunk
- [x] 3.9 新增测试：orphan queued input 无 AIChunk 时丢弃

## 4. 前端渲染

- [x] 4.1 `SessionDetail.svelte` 在 semantic steps 遍历中新增 `user_message` 分支，用 BaseItem 渲染（MESSAGE_SQUARE + "User" + summary + 展开）
- [x] 4.2 确认 `svelte-check` 与 `vitest` 通过

## 5. IPC contract

- [x] 5.1 `cdt-api/tests/ipc_contract.rs` 新增 round-trip 测试：SessionDetail 返回含 UserMessage step 的 AIChunk，验证 serde tag 为 `"user_message"` + 字段 camelCase

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

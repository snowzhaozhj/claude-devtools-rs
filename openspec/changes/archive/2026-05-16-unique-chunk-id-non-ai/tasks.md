## 1. cdt-analyze: chunk_id 去重 helper

- [x] 1.1 在 `crates/cdt-analyze/src/chunk/builder.rs` 加 `fn next_non_ai_chunk_id(uuid: &str, ordinals: &mut HashMap<String, usize>) -> String`——首次返回裸 `uuid.to_owned()`，后续返回 `format!("{uuid}:{count}")`，内部 `*count += 1`
- [x] 1.2 在 `build_chunks` / 等 callsite 上下文中新增 `non_ai_chunk_ordinals: HashMap<String, usize>` 局部变量，与 `ai_chunk_ordinals` 平级穿过 build pipeline
- [x] 1.3 替换 `out.push(Chunk::User(UserChunk { chunk_id: msg.uuid.clone(), ... }))` 两处（slash user 分支 + 普通 user 分支）→ 调 helper
- [x] 1.4 替换 `out.push(Chunk::System(SystemChunk { chunk_id: msg.uuid.clone(), ... }))`（local-command stdout 分支）→ 调 helper
- [x] 1.5 替换 `out.push(Chunk::Compact(CompactChunk { chunk_id: msg.uuid.clone(), ... }))` → 调 helper
- [x] 1.6 codex CR 兜底（design D1b）：把 `ai_chunk_ordinals` + `non_ai_chunk_ordinals` 两个 HashMap 合并为单一 `used_chunk_ids: HashSet<String>`；`next_ai_chunk_id` / `next_non_ai_chunk_id` 都从 set 校验 candidate 已未被占用，命中冲突时 while loop 递增 ordinal 直到不撞

## 2. cdt-analyze: 测试

- [x] 2.1 在 `crates/cdt-analyze/src/chunk/builder.rs` 加 `duplicate_user_uuid_gets_stable_unique_chunk_ids` 测试：构造两条 `uuid == "u-dup"` 的 user 消息（中间夹 assistant 模拟 bg 回放），断言产出 2 个 `UserChunk`，第一个 `chunk_id == "u-dup"`、第二个 `chunk_id == "u-dup:1"`
- [x] 2.2 跑 `cargo test -p cdt-analyze`，确认既有 `duplicate_assistant_response_uuid_gets_stable_unique_chunk_ids` 仍绿、新增 user 测试绿
- [x] 2.3 codex CR 兜底：加 `user_uuid_collides_with_suffix_form_still_unique` 测试——fixture 含 uuid=`abc` 与 uuid=`abc:1` 同 session 共存，断言 `abc` 第二次产 chunk_id=`abc:2`（跳过已被占的 `abc:1`）

## 3. ui: 前端 expand state 切到 chunkId

- [x] 3.1 codex CR Bug 3：`ui/src/routes/SessionDetail.svelte` 的 `expandedCompacts` set 改用 `chunk.chunkId` 而非 `chunk.uuid`（`toggleCompact` 参数名 + `@const isCompactExpanded` + `onclick` 三处）——避免同 uuid 两个 compact chunk 共享展开状态

## 4. spec + 验证

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.2 `cargo fmt --all`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `pnpm --dir ui run check`
- [x] 4.5 `openspec validate unique-chunk-id-non-ai --strict`

## 5. 发布

- [x] 5.1 push 分支 + 开 PR（与 release bump 同 PR `chore/release-0.5.1`）
- [x] 5.2 wait-ci 全绿
- [ ] 5.3 codex 二审通过（如发现 bug：修 → push → 回到 5.2 重跑；可循环 M 次）
- [ ] 5.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

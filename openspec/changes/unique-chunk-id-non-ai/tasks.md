## 1. cdt-analyze: chunk_id 去重 helper

- [x] 1.1 在 `crates/cdt-analyze/src/chunk/builder.rs` 加 `fn next_non_ai_chunk_id(uuid: &str, ordinals: &mut HashMap<String, usize>) -> String`——首次返回裸 `uuid.to_owned()`，后续返回 `format!("{uuid}:{count}")`，内部 `*count += 1`
- [x] 1.2 在 `build_chunks` / 等 callsite 上下文中新增 `non_ai_chunk_ordinals: HashMap<String, usize>` 局部变量，与 `ai_chunk_ordinals` 平级穿过 build pipeline
- [x] 1.3 替换 `out.push(Chunk::User(UserChunk { chunk_id: msg.uuid.clone(), ... }))` 两处（slash user 分支 + 普通 user 分支）→ 调 helper
- [x] 1.4 替换 `out.push(Chunk::System(SystemChunk { chunk_id: msg.uuid.clone(), ... }))`（local-command stdout 分支）→ 调 helper
- [x] 1.5 替换 `out.push(Chunk::Compact(CompactChunk { chunk_id: msg.uuid.clone(), ... }))` → 调 helper

## 2. cdt-analyze: 测试

- [x] 2.1 在 `crates/cdt-analyze/src/chunk/builder.rs` 加 `duplicate_user_uuid_gets_stable_unique_chunk_ids` 测试：构造两条 `uuid == "u-dup"` 的 user 消息（中间夹 assistant 模拟 bg 回放），断言产出 2 个 `UserChunk`，第一个 `chunk_id == "u-dup"`、第二个 `chunk_id == "u-dup:1"`
- [x] 2.2 跑 `cargo test -p cdt-analyze`，确认既有 `duplicate_assistant_response_uuid_gets_stable_unique_chunk_ids` 仍绿、新增 user 测试绿

## 3. spec + 验证

- [x] 3.1 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 3.2 `cargo fmt --all`
- [x] 3.3 `cargo test --workspace`
- [x] 3.4 `pnpm --dir ui run check`
- [x] 3.5 `openspec validate unique-chunk-id-non-ai --strict`

## 4. 发布

- [ ] 4.1 push 分支 + 开 PR（与 release bump 同 PR `chore/release-0.5.1`）
- [ ] 4.2 wait-ci 全绿
- [ ] 4.3 codex 二审通过（如发现 bug：修 → push → 回到 4.2 重跑；可循环 M 次）
- [ ] 4.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

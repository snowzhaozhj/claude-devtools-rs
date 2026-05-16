## 1. cdt-core / cdt-analyze chunk identity

- [x] 1.1 在 `cdt-core` 的所有 `Chunk` variant 类型中增加 `chunk_id` 字段并保持 camelCase 序列化为 `chunkId`
- [x] 1.2 在 `cdt-analyze` chunk builder 中为 `UserChunk` / `SystemChunk` / `CompactChunk` 填充自身 uuid 作为 `chunk_id`
- [x] 1.3 在 `cdt-analyze` chunk builder 中为 `AIChunk` 生成 `firstResponseUuid + occurrenceOrdinal` 形态的稳定唯一 `chunk_id`
- [x] 1.4 补齐所有 Rust 测试 fixture / struct literal 构造点，确保 workspace 编译通过

## 2. cdt-api IPC contract

- [x] 2.1 在 `get_session_detail` 共享路径确认 `SessionDetail.chunks[*].chunkId` 通过 IPC / HTTP 序列化透出
- [x] 2.2 在 `crates/cdt-api/tests/ipc_contract.rs` 增加 `chunkId` camelCase contract 断言
- [x] 2.3 增加重复 assistant response uuid 的回归测试，断言两个 `AIChunk.chunkId` 唯一且重复调用稳定
- [x] 2.4 运行 `cargo test -p cdt-api --test ipc_contract`

## 3. SessionDetail 前端迁移

- [x] 3.1 同步 `ui/src/lib/api.ts` Chunk 类型新增 `chunkId`
- [x] 3.2 更新 `ui/src/lib/__fixtures__` 与 `tauriMock` detail fixtures 填充 `chunkId`
- [x] 3.3 将 `ui/src/routes/SessionDetail.svelte` 顶层 keyed each、chunk 级展开状态、chunk DOM 标记迁移到 `chunkId`
- [x] 3.4 检查滚动保存与 `openOrReplaceTab` guard，补回归测试防止旧 session 状态污染新 session
- [x] 3.5 检查搜索定位 / `contentVersion` 刷新路径，确保 chunk 级定位使用 `chunkId`

## 4. UI 测试与验证

- [x] 4.1 更新现有 duplicate response uuid Vitest 回归测试，断言 `chunkId` key 不崩溃
- [x] 4.2 补充展开状态或滚动保存相关 Vitest / e2e 覆盖
- [x] 4.3 运行 `npm run check --prefix ui`
- [x] 4.4 运行 `npm run test:unit --prefix ui`
- [x] 4.5 运行相关 e2e 或 mock browser smoke 验证 SessionDetail 渲染与搜索 golden path

## 5. OpenSpec 与发布尾段

- [x] 5.1 运行 `openspec validate session-detail-stable-chunk-id --strict`
- [ ] 5.2 push 分支 + 开 PR
- [ ] 5.3 wait-ci 全绿
- [ ] 5.4 codex 二审通过（如发现 bug：修 → push → 回到 5.3 重跑）
- [ ] 5.5 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

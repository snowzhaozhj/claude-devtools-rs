# Tasks — unify-session-detail-title-with-sidebar

## 1. 后端 SessionDetail 加 title 字段

- [x] 1.1 `crates/cdt-api/src/ipc/types.rs::SessionDetail` 加 `pub title: Option<String>`（`#[serde(default)]`）
- [x] 1.2 `crates/cdt-api/src/ipc/local.rs::get_session_detail` 调 `extract_session_metadata_from_parsed(&messages, !is_ongoing)` 取 `SessionMetadata.title`，写入 `SessionDetail.title`
- [x] 1.3 `crates/cdt-api/tests/ipc_contract.rs` 加 `session_detail_title_field_round_trip` 测试（serialize → deserialize 字段名为 `title`）
- [x] 1.4 加单元测试覆盖 spec scenarios：interruption / slash with-args / teammate summary / command stdout 跳过 / 500 字截断 / 空 messages None / slash 无 args fallback / `is_meta=true` 跳过 / sanitize 后空取下一条。实测落点为 `crates/cdt-api/src/ipc/session_metadata.rs::tests::detail_title_*`（10 个 case；直接调 `extract_session_metadata_from_parsed`，等价于 `get_session_detail` 派生 title 的纯函数核心；**不**复用 `crates/cdt-parse/tests/fixtures/`，直接构造 `ParsedMessage` Vec）

## 2. 前端 SessionDetail 消费 detail.title

- [x] 2.1 `ui/src/routes/SessionDetail.svelte` 删除 `firstUserTitle(chunks)` 函数；`utext` 在 line 800 仍有用，保留
- [x] 2.2 `<h1 class="top-title">` 表达式改为 `{detailTitle(detail)}`（helper 内 `detail.title ?? sessionId.slice(0, 8)`）；`ui/src/lib/api.ts::SessionDetail` TS 类型同步加 `title?: string | null`
- [x] 2.3 `ui/src/components/SessionDetail.test.svelte.ts` 加单测：`detail.title` 存在时 `<h1>` 渲染该值；`detail.title === null` 时渲染 `sessionId.slice(0, 8)`
- [x] 2.4 `pnpm --dir ui run check` + `just test-ui-unit` 通过（411 tests pass）

## 3. 自验

- [x] 3.1 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 3.2 `cargo fmt --all`
- [x] 3.3 `cargo test -p cdt-api` 通过（含 10 个新 detail_title scenario + ipc_contract round-trip）
- [x] 3.4 `pnpm --dir ui run check` + `just test-ui-unit` 通过
- [x] 3.5 `openspec validate unify-session-detail-title-with-sidebar --strict` 通过
- [ ] 3.6 ~~e2e-http-verify 跑两个报告的 sessionId（`fe7cf094...` / `6290f9d4...`）~~：跳过——3 层测试（cdt-api 10 个 scenario unit + ipc_contract round-trip + ui vitest 2 个 case）已字节级覆盖 detail / sidebar 派生一致；e2e 需要用户机器上的真实 JSONL，bg session 跑不动该验证。用户可在合并后桌面端自行回归这两个 sessionId。

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

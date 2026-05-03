## 1. 后端 messageCount 算法对齐 isParsedUserChunkMessage

- [x] 1.1 `crates/cdt-api/src/ipc/session_metadata.rs` 加 `is_user_chunk_message(msg: &ParsedMessage) -> bool` 私有函数：检查 category != User 或 is_meta → false；Text content → true；Blocks → 至少含一个 Text 或 Image block 才返回 true
- [x] 1.2 `extract_session_metadata` 计数判断从 `category == User && !is_meta` 改为 `is_user_chunk_message(&msg)`
- [x] 1.3 `cargo clippy -p cdt-api --all-targets -- -D warnings` 全绿
- [x] 1.4 `cargo fmt --all`

## 2. 后端 messageCount 单测覆盖

- [x] 2.1 `session_metadata::tests` 加 `tool_result_only_user_row_not_counted` 单测：构造 user/assistant tool_use/user tool_result/assistant 4 行，断言 messageCount=2
- [x] 2.2 加 `mixed_text_and_tool_result_user_row_counted`：含 text+tool_result blocks 的 user 行计入
- [x] 2.3 加 `image_only_user_row_counted`：只含 image block 的 user 行计入
- [x] 2.4 加 `is_meta_user_row_not_counted`：is_meta=true 的 user 行不计入
- [x] 2.5 `cargo test -p cdt-api` 全绿

## 3. 前端 SessionItem meta 行加 git 分支 chip

- [x] 3.1 `ui/src/components/Sidebar.svelte` SessionItem 第二行 meta 末尾追加 `{#if session.gitBranch}<span class="session-meta-sep">·</span><span class="session-branch"><svg>{@html GIT_BRANCH_SVG}</svg>{session.gitBranch}</span>{/if}`
- [x] 3.2 加 `.session-branch` CSS：inline-flex + gap 2px + 10px 字号 + ellipsis 截断
- [x] 3.3 git 分支 icon 颜色用 `rgba(52, 211, 153, 0.7)`（与 SidebarHeader 之前那栏一致）

## 4. 前端 SidebarHeader 去掉 branch row

- [x] 4.1 `ui/src/components/SidebarHeader.svelte` 删除 `branch-row` 渲染块、`branch` $derived、`sessions` 与 `activeSessionId` props 与对应 import（`SessionSummary` import / `GIT_BRANCH_SVG` import）
- [x] 4.2 删除 `.branch-row` / `.branch-icon` / `.branch-name` CSS 块
- [x] 4.3 `Sidebar.svelte` 不再透传 `{sessions}` / `{activeSessionId}` 给 SidebarHeader

## 5. 前端 e2e 调整

- [x] 5.1 `ui/tests/e2e/sidebar-collapse-and-branch.spec.ts` 第三个 test "git 分支栏渲染 active session 的 gitBranch" 改名为 "git 分支 chip 在每条 SessionItem 行内显示"
- [x] 5.2 断言改为：在 SessionItem 列表内能找到含 `feat/frontend-test-infrastructure` 文本的 git chip 元素（fixture 中 `sess-rust-active` 该值），同时另一条 session（`sess-rust-2`）显示 `main`
- [x] 5.3 切 session 跟随更新断言可去掉（per-session chip 静态显示，不需要 active 切换响应）

## 6. 验证 + 归档

- [x] 6.1 `npm run check --prefix ui` 全绿
- [x] 6.2 `npm run test:unit --prefix ui` 全绿
- [x] 6.3 `npx playwright test sidebar-collapse-and-branch.spec.ts` 全绿
- [x] 6.4 `just preflight` 全绿
- [x] 6.5 `openspec validate sidebar-meta-row-fix --strict` 全绿
- [ ] 6.6 codex:codex-rescue 二审（行为契约改动 + 算法修订，按 `.claude/rules/codex-usage.md` 必跑）
- [ ] 6.7 修完先跑第二轮 codex 验证才 push，archive commit 是验证通过后才打的 PR 最后一个 commit
- [ ] 6.8 archive：`openspec archive sidebar-meta-row-fix -y`
- [ ] 6.9 commit + push 到 PR #38（不开新分支）

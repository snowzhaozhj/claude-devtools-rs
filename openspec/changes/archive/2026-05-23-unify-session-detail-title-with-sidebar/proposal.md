## Why

会话详情页（`SessionDetail.svelte`）顶部 `<h1>` 调用前端独立函数 `firstUserTitle(chunks)` 派生标题，与 sidebar 列表项消费的 `SessionSummary.title`（后端 `extract_session_metadata` 派生）规则严重分叉，至少 6 处不一致：

1. **slash with args**：sidebar 把 `/model sonnet` 当作 title；detail 跳过所有 `/` 起首消息
2. **interruption 过滤**：sidebar 跳过 `[Request interrupted by user`；detail 不过滤
3. **teammate summary**：sidebar 提取 `<teammate-message summary="...">` 的 summary；detail `cleanDisplayText` 把 teammate-message 整段抹空
4. **command output**：sidebar 跳过 `<local-command-stdout>`；detail 反而把 stdout 内容**当作** title 抽出
5. **截断长度**：sidebar 500 字（spec `ipc-data-api::TITLE_MAX_CHARS`）；detail 60 字 + `...`
6. **fallback**：sidebar `sessionId.slice(0, 8)`；detail `sessionId.slice(0, 12)`

复现 sessionId：`fe7cf094-5c3e-48c1-b891-e72551de0bb4` / `6290f9d4-c982-4ec8-89c7-5c6de88fad1a`——两个 session 在 sidebar 和 detail 显示的 title 不同。

根因：`SessionDetail` IPC 没暴露 `title` 字段，frontend 被迫自派生 → 双源分叉。

## What Changes

- **后端 `cdt-api`**：`SessionDetail` struct 新增 `title: Option<String>` 字段（camelCase 序列化）；`get_session_detail` 复用已有的 `messages: &[ParsedMessage]` 调 `extract_session_metadata_from_parsed` 一次填入，0 额外 I/O
- **前端 `ui/`**：`SessionDetail.svelte` 删除 `firstUserTitle(chunks)` 函数；`<h1>` 直接渲染 `detail.title ?? sessionId.slice(0, 8)`，与 sidebar fallback 长度对齐
- **spec delta**：`ipc-data-api/spec.md` 新增 Requirement「`SessionDetail.title` 与 `SessionSummary.title` 共用单一派生源」，写明 detail 端 title 派生与 sidebar 字节级一致

**BREAKING**：无（新增字段；老前端忽略 `title` 字段时 `firstUserTitle` 已下线，但后端字段是新增）。

## Impact

- Affected specs: `ipc-data-api`
- Affected code:
  - `crates/cdt-api/src/ipc/types.rs::SessionDetail`（+1 字段）
  - `crates/cdt-api/src/ipc/local.rs::get_session_detail`（+1 次 `extract_session_metadata_from_parsed` 调用）
  - `crates/cdt-api/tests/ipc_contract.rs`（+1 round-trip 测试）
  - `crates/cdt-api/tests/get_session_detail*.rs`（覆盖 6 个分叉规则）
  - `ui/src/routes/SessionDetail.svelte`（-`firstUserTitle` 函数 / 改 `<h1>` 表达式）
  - `ui/src/components/SessionDetail.test.svelte.ts`（断言 `detail.title` 渲染）
- Perf：`extract_session_metadata_from_parsed` 是纯 sync iter，输入是已 parsed messages，复杂度 `O(min(n, 200))`——首屏 IPC 增量 ≤ 1ms，可忽略

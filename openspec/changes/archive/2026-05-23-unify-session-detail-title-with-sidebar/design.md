# Design — unify-session-detail-title-with-sidebar

## Context

**当前状态**：
- Sidebar 列表项标题字段：`SessionSummary.title: Option<String>`，由 `extract_session_metadata`（`crates/cdt-api/src/ipc/session_metadata.rs:147`）扫 JSONL 前 200 行后派生；spec `ipc-data-api` 已规范规则（500 字截断 / 7 类 system tag 过滤 / interruption 跳过 / teammate-message summary 提取 / slash with-args 直接当 title）
- SessionDetail 顶部 `<h1>` 标题：前端 `firstUserTitle(chunks)`（`ui/src/routes/SessionDetail.svelte:687`）独立派生，逻辑严重偏离 backend

**问题**：用户在同一 session 的 sidebar 项和打开后的详情页头看到不同标题。

**已有可复用的基础设施**：`extract_session_metadata_from_parsed(messages: &[ParsedMessage], is_stale: bool) -> SessionMetadata`（`session_metadata.rs:295`）——纯 sync 函数，从已 parsed messages Vec 直接派生 title。`get_session_detail` 已经在调用链内持有 `messages: Vec<ParsedMessage>` 全文件解析结果，复用零开销。

## Decisions

### D1：title 派生源由后端单点持有，frontend 不再自派生

**选项**：
- **(A) 后端在 `SessionDetail` IPC 加 `title` 字段，frontend 直接消费**（选这个）
- (B) frontend 在 `SessionDetail.svelte` 端模仿 `extract_session_metadata` 的 6 条规则
- (C) frontend 通过 `sessionListStore` 反查同 sessionId 已 cache 的 `SessionSummary.title`

**选 A 理由**：
- 单一真相源。后端规则未来扩展（如 slash 命令新形态、新 system tag）frontend 自动跟上，不会再分叉
- B 需要把 `sanitize_for_title` 的 7 类 tag 名单 + interruption prefix + teammate summary 正则三处常量在 frontend 复制——`openspec/specs/ipc-data-api/spec.md` 已硬约束 backend 是真相源，复制就是反 spec
- C 在 search 直接打开非当前 project session 时 `sessionListStore` 没数据；fallback 链复杂

**代价**：IPC payload 加 1 个 `Option<String>`，最长 500 字（已 truncate），相对 chunks/metrics MB 量级可忽略；后端多调一次 `extract_session_metadata_from_parsed`（O(min(n, 200)) 纯 sync），首屏 IPC 增量 ≤ 1ms（sub-ms）。

### D2：`get_session_detail` 复用已 parsed `messages`，不重读 JSONL

`get_session_detail` 在 line 3076 已经拿到 `messages: Vec<ParsedMessage>`（`extract_parsed_messages_cached`）。`extract_session_metadata_from_parsed(&messages, is_stale)` 仅消费这个 slice，**不**触发任何 fs 调用（实现仅 `messages.iter()` + 纯文本 helper，无 await）。

**`is_stale` 入参选择**：本 change 只消费返回值的 `title` 字段，`is_stale` 仅参与 `is_ongoing` 派生不影响 `title`，理论上可传任意值。为对齐 sidebar 派生函数 signature（避免日后误读"detail 端 stale 语义不一致"），本 change 传 `!is_ongoing` —— 让 `SessionMetadata.is_ongoing` 与 detail 已算出的 `is_ongoing` 等价。**注**：`!is_ongoing` ≠ 真实 fs stale 状态（sidebar 真实 stale check 在 `extract_session_metadata_with_ongoing` line 276-283 中由 `is_file_stale(fs, path)` 计算）；本 change 传该值仅取等价 `is_ongoing` 输出，**不**用作 stale 判定真相。

### D3：fallback 长度统一到 sidebar 的 8 字符

frontend 当前 fallback `sessionId.slice(0, 12)`，sidebar 是 `sessionId.slice(0, 8)`。本 change 选 **8**——sidebar 与 spec `ipc-data-api` 已固定 8，detail 跟随。

### D-V1：`<h1>` 视觉零变化

仅替换 `<h1>{firstUserTitle(detail.chunks)}</h1>` → `<h1>{detail.title ?? sessionId.slice(0, 8)}</h1>`。CSS `.top-title` 不动；500 字超长由 backend 已截断兜底，frontend 不加额外 truncate（sidebar 是 `text-overflow: ellipsis`，detail 顶 bar 也已通过 `.top-title` 容器宽度自然处理过长——不在本 change 范围）。

## Open Questions

无。

## Risks

- **R1**：老前端 bundle（不消费新字段）会忽略 `title`，沿用 `firstUserTitle`——这是过渡期可接受降级，且我们前端 SHALL 同 PR 升级，无 staged rollout 场景
- **R2**：6 个 sessionId fixture（含报告的 fe7cf094 / 6290f9d4）测试要 mock 出消息里特定 system tag / interruption / teammate summary 形态——fixture 在 `crates/cdt-parse/tests/fixtures/` 已有齐全模式可复用

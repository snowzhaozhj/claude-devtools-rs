# Design: cleanup-purity-ipc-data-api

## Decisions

### D1: 替换策略——模块路径 → 行为角色描述

mod-path（如 `cdt_analyze::check_messages_ongoing`、`DataApi::list_sessions`、`broadcast::Sender`）替换为行为角色描述（"活动栈判定函数"、"会话列表查询接口"、"事件广播通道"）。

候选方案：
- A: 去掉所有内部名（彻底纯净但 reviewer 难对应代码）
- B: 保留逻辑描述但不写具体模块/函数名（✓ 选定）

理由：spec 是行为契约不是 API docs；reviewer 靠 Scenario 验证，不靠模块名。

### D2: 源码路径 → 删除或改为角色

`crates/cdt-api/tests/ipc_contract.rs` → "IPC contract test 套件"；`local.rs:3243` → "历史 wire 形状"。

### D3: commit/PR/issue 引用 → 删除

`PR #291`、`issue #94`、`` `46a25772` `` 等删除。行为动机保留在 spec 正文（如"CLI 异常退出时活动栈误判 ongoing"），不需要引用具体 issue/commit。

### D4: 实测数值 → 删除或改为定性

"实测 `46a25772` case 下 1257 KB / 41%" → 删除（payload 瘦身的量化预算在 `.claude/rules/perf.md` 而非 spec 内）。"~10µs" → 删除。

### D5: 回滚开关 const → 行为描述

`` `OMIT_RESPONSE_CONTENT: bool` `` → "response content 裁剪开关"。注意 `OMIT_TOOL_OUTPUT=true` 在 Scenario 标题里是上下文条件描述可保留为逻辑条件名。

### D6: 库/框架引用 → 行为描述

`tokio::fs::metadata` → "异步文件 stat 调用"；`broadcast::Sender` → "事件广播通道"；`serde_json::to_value` → "JSON 序列化"；`axum handler` → "HTTP handler"；`Arc<Semaphore>` → "共享并发限制器"。

### D7: 保留的外部协议名

以下是外部可见协议 / IPC wire 形态，保留不改：
- Tauri command 名（`get_session_detail`、`list_sessions` 等）
- IPC 字段名 camelCase（`messagesOmitted`、`outputBytes`、`contentOmitted` 等）
- `xxxOmitted` flag 语义
- Scenario 标题中的逻辑条件名（`OMIT_TOOL_OUTPUT`）
- HTTP 路由路径（`GET /api/projects/{projectId}/sessions`）
- 用户感知阈值（"5 分钟 stale"）

## Risk

零风险——纯文字重写，不改行为；`openspec validate` 仍过。

## Why

`extract_session_metadata_with_ongoing` 在 cache miss 时逐行解析 JSONL 后，把所有 `ParsedMessage` push 进 `Vec<ParsedMessage>`，仅为最后调一次 `cdt_analyze::check_messages_ongoing(&messages)` 拿 ongoing 判定。

后台 metadata scan 通过 `Semaphore(8)` 并发执行：8 个大会话（典型 10k 消息 / ParsedMessage Vec 估算 20MB+ 含 blocks 内嵌 JSON 与 string）同时 in-flight 时，进程驻留峰值 ≈ **160 MB+**，单纯为了"算一个 bool"。

`check_messages_ongoing` 内部算法（`build_activity_stack` + `is_ongoing_from_activities`）在每条消息上只需要常量级状态——它本质是个状态机，把活动栈算法重写成增量 `feed(msg)` 接口后，metadata 提取链路就能丢掉 `all_messages` Vec，把内存复杂度从 O(N) 降到 O(1)。

## What Changes

- **新增** `cdt_analyze::session_state::IsOngoingStateMachine`：增量状态机，接口 `new() / feed(&mut self, msg: &ParsedMessage) / finalize(self) -> bool`。状态字段：单 `ongoing: bool` + `shutdown_tool_ids: HashSet<String>`（追踪 SendMessage shutdown_response 的 tool_use_id，与现有 `process_assistant` 一致）
- **重写** `cdt_analyze::check_messages_ongoing(&[ParsedMessage]) -> bool` 为 SM 的 thin wrapper：内部 `for msg in messages { sm.feed(msg) } sm.finalize()`，公开 API 签名不变
- **保留** 原 `build_activity_stack` + `is_ongoing_from_activities` 函数 **作为 `#[cfg(test)]` oracle**（不公开 API），用于 round-trip test 在多种 fixture 上断言"SM 与活动栈算法等价"
- **改造** `crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_with_ongoing`：parse loop 内对每条解析出的 `ParsedMessage` 即时 `sm.feed(&msg)`，删除 `all_messages: Vec<ParsedMessage>` 字段，loop 结束后调 `sm.finalize()` 拿 `messages_ongoing`
- **新增 round-trip test**：`crates/cdt-analyze/src/session_state.rs` 内 fixture-driven property test，覆盖 normal / ongoing / stale / interrupted / teammate / completed 六类典型场景，断言 SM 与 oracle 算法在每个 fixture 上结果一致

非变更项：
- 公开 API 签名（`check_messages_ongoing(&[ParsedMessage]) -> bool` 不变）
- spec 中 `isOngoing` 真实值的两路 AND 计算定义（`messages_ongoing && !is_session_stale`）保持不变
- IPC payload / Tauri command 协议 / camelCase / `MetadataCache` / `SessionMetadataUpdate` 全部不动
- `STALE_SESSION_THRESHOLD = 5 min` 阈值不变
- `extract_session_metadata` 公开签名 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata` 不变（spec line 893 SHALL 保护）

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `ipc-data-api`：在 `Requirement: extract_session_metadata 按 FileSignature 缓存` 块之后**新增** Requirement `extract_session_metadata 流式判定 isOngoing`，固化"不收集全量 ParsedMessage Vec / O(1) 内存 / 与切片版等价"三条 SHALL

## Impact

- 代码：
  - `crates/cdt-analyze/src/session_state.rs`：重构为 SM 主路径 + activity-stack `#[cfg(test)]` oracle
  - `crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_with_ongoing`：删 `all_messages` Vec，改流式喂 SM
- 测试：
  - `crates/cdt-analyze/src/session_state.rs#tests`：新增 round-trip property test（含 6 类典型 fixture）
  - 既有 9 个 `check_messages_ongoing` 单元测试自动通过新 SM 路径（thin wrapper）
- 性能：
  - cache miss 8 路并发场景峰值内存：~160MB → O(8 × SM 自身字段) ≈ KB 级
  - CPU 路径不变（SM 与 activity-stack 复杂度等价 O(N)），cold scan / get_session_detail bench 无回归
- 用户体感：发版后大量大会话项目首次冷启动时驻留内存下降——**间接**改善 OS 调度（不再触发 swap）
- spec：`openspec/specs/ipc-data-api/spec.md` 新增一条 Requirement 块（在 cache 那条之后）

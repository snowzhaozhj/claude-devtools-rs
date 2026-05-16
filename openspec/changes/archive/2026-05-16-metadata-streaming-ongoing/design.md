# Design

## Context

参考 PR proposal `feedback_align_with_original.md` + `.claude/rules/perf.md` 的"反模式严禁清单"："hot path 缺 cache" + "每次 IPC 重扫不 cache" 之外，**驻留内存峰值**也是同一类反模式——尤其当 ① workload 结构上线性可流式（一次遍历足够），② 数据本体（`ParsedMessage`）每条 KB 级，③ 并发度 8 把单条尺寸放大 8x。

`extract_session_metadata_with_ongoing` 同时命中三个条件：
1. JSONL 已经在 `tokio::io::BufReader::lines()` 流式读，逐行 push 进 `all_messages` 后**只调用一次** `check_messages_ongoing`
2. `ParsedMessage` 含 `MessageContent::Blocks(Vec<ContentBlock>)`，blocks 里的 `ToolUse.input` / `ToolResult.content` 是 `serde_json::Value`，typical 大会话每条 1–5 KB
3. cache miss 路径同时跑 8 路（`metadata_scan_semaphore`）

## Decisions

### D1：增量状态机替代切片算法

把 `cdt_analyze::session_state::check_messages_ongoing(&[ParsedMessage]) -> bool` 重写成 `IsOngoingStateMachine`：

```rust
pub struct IsOngoingStateMachine {
    ongoing: bool,
    shutdown_tool_ids: HashSet<String>,
}

impl IsOngoingStateMachine {
    pub fn new() -> Self { ... }
    pub fn feed(&mut self, msg: &ParsedMessage) { ... }
    pub fn finalize(self) -> bool { self.ongoing }
}
```

#### 候选方案对比

- **(a) 完全流式无 Vec**（采纳）—— SM 内部仅常量级状态，不保留任何过往 ParsedMessage 副本。每条消息触发 0..N 次"事件"（一条消息可能贡献多个 block），每个事件按"AI 活动 / ending"两类直接更新 `ongoing` bool
- **(b) 保留最后 N 条消息的 sliding window**：原算法语义上"找最后 ending 之后是否还有 AI 活动"，理论上只需要追踪从最后 ending 起后的活动。但每个 block 是独立事件，N 没有自然上界，sliding window 复杂度反而比单 bool 高
- **(c) 保留 activity-stack `Vec<Activity>` 但增量 push**：相对原版只省 `Vec<ParsedMessage>` 不省 `Vec<Activity>`，节省幅度有限（Activity 是 enum 1 字节），又增加了状态机内部容器复杂度

选 (a) 最彻底——把"找最后 ending 之后是否有 AI 活动"翻译为：每个事件按时间序更新单 bool，最终值即为答案。等价证明见下面 D4。

### D2：状态字段保持最小

只两个字段：
- `ongoing: bool`：当前 running 判定
- `shutdown_tool_ids: HashSet<String>`：追踪 SendMessage shutdown_response 的 tool_use_id，让后续 user 消息看到对应 tool_result 时正确判定为 Interruption（与现有 `process_assistant` 内 `shutdown_tool_ids` 完全一致）

`shutdown_tool_ids` 容量上界：1 个 session 内 shutdown_response tool_use 调用次数（实测 0–2）；`String` 是 tool_use_id（typical 24 字符）。整个 SM 单实例 < 200 字节。8 路并发 < 2KB。

### D3：parse loop 内即时喂状态机

`extract_session_metadata_with_ongoing` 的 loop body 改造：

```rust
let mut sm = IsOngoingStateMachine::new();
// ... 原 title / message_count / git_branch 提取逻辑保持
sm.feed(&msg);     // 新增一行
all_messages.push(msg);  // 删除
```

loop 结束后：
```rust
let messages_ongoing = sm.finalize();  // 替代 check_messages_ongoing(&all_messages)
```

风险：`sm.feed(&msg)` 借用 `msg` 不消费，因此**先 feed 再做后续标题提取**或反之均可。本设计选**先做完原有提取后 feed 再 push 行**——即把 `sm.feed(&msg)` 放在 push 位置之前一行，保持其它 loop 体不变。

### D4：与原算法等价证明 + round-trip test

原算法（保留为 `#[cfg(test)]` oracle）：
1. `build_activity_stack(messages)` → `Vec<Activity>`
2. 找最后一个 ending event 位置 `idx`
3. `idx + 1..` 范围内是否含任意 AI 活动

声称：每个 event 按时间序更新 `ongoing = (event.is_ai())` 后，最终 `ongoing` 与上述算法等价。

证明（归纳）：
- 每条事件按时间序处理；事件分两类：AI 活动（Thinking / ToolUse / ToolResult）与 ending（TextOutput / Interruption / ExitPlanMode）
- AI 活动 → `ongoing = true`；ending → `ongoing = false`
- 终态 `ongoing` 反映**最后一个事件**的类型（若末事件是 AI 则 true；若末事件是 ending 则 false；若无任何事件则初始 false）
- 与原算法等价的关键观察：
  - 若最后 ending 之后有 AI 活动 → 末事件必为 AI → `ongoing = true` ✓
  - 若最后 ending 之后没有 AI 活动 → 末事件必为 ending（最后 ending 本身就是末位或之后只有非分类事件，但 build_activity_stack 已过滤所有非 Activity 事件）→ `ongoing = false` ✓
  - 无 ending 但有 AI → 所有事件都是 AI → 末位 AI → `ongoing = true` ✓
  - 无任何事件 → 初始 `ongoing = false` ✓
  - 边界：空 messages → 不进 feed → 初始 false ✓

round-trip test fixture 覆盖六类：
1. **normal completed**：user → assistant text → 末位 ending → false
2. **ongoing tool-use**：user → assistant tool_use → 无 ending → true
3. **interrupted**：user → assistant tool_use → user interrupt → 末位 ending → false
4. **teammate-message**：user(teammate) → assistant tool_use → user tool_result → 末位 AI → true
5. **shutdown_response**：assistant SendMessage(shutdown,approve=true) → user tool_result(matching id) → 触发 shutdown_tool_ids 路径，末位 ending → false
6. **resumed-after-interrupt**：assistant tool_use → user interrupt → assistant tool_use → 末位 AI → true

每个 fixture 同时跑 SM 与 oracle，断言相同。

### D5：`#[cfg(test)]` oracle 的代码归宿

把 `build_activity_stack` + `is_ongoing_from_activities` + `Activity` enum + `INTERRUPT_PREFIX` 整体移入 `#[cfg(test)] mod oracle { ... }` 子模块（**不**删除）。理由：

1. **回归防护**：oracle 是已被 9 条单元测试 + 历史 archive `session-ongoing-stale-check` 验证过的算法快照；保留它让 round-trip test 能在未来 SM 改动时立即抓出回归
2. **算法可读性**：oracle 提供"先有完整活动栈再后扫"的视角，比 SM 流式版本更直观，docstring 能交叉引用
3. **维护成本可控**：oracle 不公开 / 不参与编译产物（cfg(test) 切割），不增加 release binary size

`Activity` enum 因此从 module-level 降为 `#[cfg(test)] mod oracle` 内部 enum；公共 API 完全不暴露 enum，前端 / 其他 crate 不受影响。

### D6：`is_meta` / synthetic 模型 / sidechain 是否需要 SM 内过滤

现有 `build_activity_stack` 只看 `MessageType::Assistant` / `MessageType::User`，**不**过滤 `is_meta` / `synthetic` model / sidechain——这三个过滤在 `message_count` 计数路径里做（`is_user_chunk_message`），而 ongoing 判定**与 message_count 互不耦合**。

SM 保持完全相同的入口约定：`feed(&msg)` 不查 `is_meta` / `model` / `is_sidechain`，按 `message_type` 分发到 `process_assistant` / `process_user`，与原 `build_activity_stack` 行为 1:1 等价。round-trip test 自动覆盖（fixture 内含 `is_meta=true` 行 / synthetic assistant 行 / sidechain 行的版本，行为与未含时无差异）。

## Risks & Trade-offs

| 风险 | 缓解 |
|---|---|
| SM 与 oracle 等价性证明遗漏 corner case | round-trip test 覆盖 6 类 fixture + 既有 9 个单元测试 thin-wrapper 后自动验证 oracle 路径与公共 API 路径都通过 SM；oracle 单元测试在 `#[cfg(test)]` 块内**额外**保留作为算法快照 |
| `shutdown_tool_ids` 在长会话内增长 | 实测 shutdown_response 调用 0–2 次/会话；HashSet<String> 容量自然有界 |
| 多 block 一行消息处理顺序 | 与原 `process_assistant` / `process_user` 完全一致——按 blocks Vec 顺序逐 block 处理，最后一个 block 的事件类型决定该消息对 `ongoing` 的最终贡献 |
| `extract_session_metadata` 公开签名变更 | 不变。SM 仅在内部 `extract_session_metadata_with_ongoing` 中使用 |
| 既有 `cdt_analyze::check_messages_ongoing` 调用方 | 仅 `cdt-api/src/ipc/session_metadata.rs` 一处。改 thin wrapper 后 binary-compatible |

## Migration

无 IPC / 持久化数据迁移。代码层一次性切换，PR 即生效。

## Out of Scope

- `MetadataCache` 自身（不动 capacity / FileSignature 算法）
- `is_session_stale` / `STALE_SESSION_THRESHOLD` 阈值 / wall-clock 合成 `is_ongoing` 路径（不动）
- `subscribe_session_metadata` / `SessionMetadataUpdate` / Tauri emit 链路（不动）
- `cdt-watch::FileWatcher` debounce / 频率（不动）

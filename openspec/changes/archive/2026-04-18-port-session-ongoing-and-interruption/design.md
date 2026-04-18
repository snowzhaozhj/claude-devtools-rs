## Context

原版 `checkMessagesOngoing` 需要一组按时间顺序的 activity tag（
`thinking` / `tool_use` / `tool_result` / `text_output` / `interruption`
/ `exit_plan_mode`），其中 ending 事件用于切分"之后的 AI 活动意味着仍
在进行"。Rust port 的 parse 层现在把 interrupt marker 当 hard noise 删
除，导致：

1. chunk-building 看不到 interrupt → 无法给 `AIChunk.semantic_steps`
   标记中断点
2. ongoing 判定拿不到 interruption activity → 被一次 Esc 中断后的会话
   仍会被永远判定为 ongoing（"绿点关不掉"）

必须先修这个分类问题，再端口算法。

## Decisions

### 1. Interruption 独立 `MessageCategory`，不再是 `HardNoise`

- `MessageCategory::Interruption` 新增。语义上仍属于 internal（不是普通
  用户问题），但**需要保留**到 chunk-building 以便挂 `SemanticStep`
- `HardNoiseReason::InterruptMarker` 删除，clippy 没有消费者抱怨（parse
  侧同步调整）
- `build_chunks` 的过滤条件只排除 `is_sidechain` 与 `HardNoise(_)`；
  `Interruption` 消息照常流入，但不进入 `UserChunk`——在 `builder` 的
  主 loop 里识别 → flush 当前 assistant buffer 时 push
  `SemanticStep::Interruption` 到**即将 flush 的 AIChunk**；没有活动的
  AI buffer 时当作孤立中断丢弃（原版同样不为此产出独立 chunk）

### 2. `SemanticStep::Interruption`

```rust
Interruption {
    text: String,      // 原 user message content（含 "[Request interrupted ..."）
    timestamp: DateTime<Utc>,
}
```

前端 switch `step.kind` 新增 `interruption` 分支，渲染为红色 badge + 原
文本，位于该 AIChunk 语义步骤尾部；避免新造 Chunk 类型，减少 UI 渲染
分支改动。

### 3. `check_messages_ongoing` 的形态

- 放在 `cdt-analyze::session_state`（新模块，pure sync，无 tokio）
- 签名：`pub fn check_messages_ongoing(messages: &[ParsedMessage]) -> bool`
- 实现完全 1:1 port 原版算法（activity enum + ending-index 扫描），
  另附 `shutdown_tool_ids: HashSet<&str>` 匹配 `SendMessage(shutdown_response,
  approve=true)` → 对应 `tool_result` 计入 ending
- 5 条 Scenario 全部在 `#[cfg(test)] mod tests` 里覆盖

### 4. ongoing 字段的供给路径

两处需要计算：

- **轻量路径 (`SessionSummary`)**：`session_metadata::extract_session_metadata`
  已经做全文件扫描（一行行 `parse_entry_at`），把 ParsedMessage 累积到
  一个 `Vec<ParsedMessage>` 开销为 O(消息数)，计算完标题后再调
  `check_messages_ongoing`。列表 sidebar 每次 `listSessions` 都重算，
  和标题扫描共享一次 I/O，可接受
- **重量路径 (`SessionDetail`)**：已经 `parse_file` 拿到 `Vec<
  ParsedMessage>`，直接多调一次 `check_messages_ongoing`，几乎零开销

### 5. UI 组件放置

- `OngoingIndicator.svelte` 导出两个组件：`<OngoingIndicator size
  ="sm"|"md" showLabel />`（绿点）与 `<OngoingBanner />`（底部横幅）
- Sidebar 两处 session-item（PINNED 分区 + 日期分组）各插入 `{#if
  session.isOngoing}<OngoingIndicator />{/if}` 在标题前
- SessionDetail 在现有 `conversation` 容器之后、footer 之前按
  `detail?.isOngoing` 渲染 `<OngoingBanner />`；保持 pinned-to-bottom
  滚动语义不变

### 6. Interruption step 渲染

在 SessionDetail 的 semantic step 渲染 switch 里新增：

```svelte
{:else if step.kind === 'interruption'}
  <div class="interruption-step">已被用户中断</div>
```

样式使用现有 `--color-danger` / 红色 token，参考原版
`SemanticStepInterruption` 的文案与色值。

## Risks / Trade-offs

- **所有 sidebar session 的 ongoing 扫描增加 I/O 耗时**：单 session
  仍然只扫一次 JSONL，但消息越多越慢。对大 session 可考虑把 `is_ongoing`
  的计算只扫尾部 N 条——本 change 先跑完整扫描以保证和 detail 一致，
  性能问题留作 follow-up（如果上线观测有感）
- **`HardNoiseReason::InterruptMarker` 删除** 属破坏性改动：仅 `cdt-
  parse`/`cdt-analyze` 内部使用，grep 过无跨 crate 消费者，安全
- **端口算法与 TS 的等价性**：完全按 TS 代码的分支 1:1 翻译，测试覆盖
  原版注释里提到的 5 种 ending 信号；如果 TS 后续修了 bug，按 followups
  常规流程补港口测试

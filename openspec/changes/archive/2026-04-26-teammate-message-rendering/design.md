## Context

当前 multi-agent / team session 在 UI 上有两个体验断层：

1. **Sidebar 标题污染**：`cdt-api::session_metadata::sanitize_for_title` 的标签剥离白名单缺 `teammate-message`，标题直接吐出原始 XML。
2. **主面板看不到队友发言**：`cdt-analyze::chunk::builder` 在 `MessageCategory::User` 分支里检测到 teammate 消息后直接 `continue` 整条丢弃，IPC 层从此完全不暴露。

数据层资产已基本就绪：`cdt-analyze::team::detection::parse_teammate_attrs` 能从 `<teammate-message teammate_id="..." color="..." summary="...">body</teammate-message>` 抽出全部字段；`Process.team` 的 enrichment 链路已跑通。原版 TS 的呈现策略（见 `claude-devtools/src/renderer/components/chat/items/TeammateMessageItem.tsx` + `displayItemBuilder.ts::linkTeammateMessagesToSendMessage`）是把 teammate 消息**嵌入**到触发它的 AI turn 内部展示流，与 `SendMessage` 调用紧邻渲染，强化"派活 → 回信"的因果配对。本 change 把这套呈现移植到 Rust port，并把数据层的"过滤"语义改为"产 sub-item"。

## Goals / Non-Goals

**Goals:**

- AIChunk 暴露新字段 `teammateMessages: Vec<TeammateMessage>`，每条携带 `teammate_id / color / summary / body / timestamp / reply_to_tool_use_id / token_count / is_noise / is_resend`，让前端能把 teammate 卡片紧邻挂在配对 SendMessage tool execution 之后渲染。
- Reply-to 配对算法：teammate 消息向前扫描最近一条 `tool_name == "SendMessage"` 且 input.recipient 匹配 `teammate_id` 的未配对 tool_use，记录其 `tool_use_id` 到 `reply_to_tool_use_id`。配对失败（孤儿）→ 字段为 None，UI 追加到 turn 末尾。
- 噪声检测：`teammate_id == "system"` 或 body 是 JSON 且 `type` 在 `idle_notification / shutdown_request / shutdown_approved / teammate_terminated` 集合内 → `is_noise = true`，UI 渲染极简单行不开卡。
- Resend 检测：summary 或 body 前 300 字符匹配 `resend / re-send / sent earlier / already sent / sent in my previous` 任一正则 → `is_resend = true`，UI 卡片半透明 + RefreshCw 图标。
- Sidebar 标题修复：`sanitize_for_title` 把 `<teammate-message>` 加入剥离集合，并新增 fast-path：消息整体被一个 teammate-message 包裹时优先取 `summary=` 属性作为标题候选。
- 前端新建 `TeammateMessageItem.svelte`，在 SessionDetail 的 AIChunk 渲染流中按 `reply_to_tool_use_id` 紧贴对应 ExecutionTrace 节点之后插入。
- 全程保留回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`，关掉等价于回退到旧"丢弃"行为。

**Non-Goals:**

- 不引入新顶层 `Chunk::Teammate` variant（保持四类 chunk 不变，避免 UI 类型 union 扩散与 search-extract / metrics 等外围逻辑改动）。
- 不改 `UserChunk` 行为：teammate 消息**仍不**产 `UserChunk`。
- 不把 `cdt-analyze::team` 检测逻辑搬到 `cdt-core`——保持现有 crate 边界，`AIChunk` 字段类型 `TeammateMessage` 放 `cdt-core` 即可（chunk 类型本身在 `cdt-core` 定义，team detection 逻辑在 `cdt-analyze`）。
- 不动 `is_meta` / slash / interruption 等已有 user 消息分支——它们的 chunk 边界含义保持不变。
- 不实现 reply-to chip 的"hover 高亮 SendMessage"反向联动（原版有，本期 UI 上保留 chip 文案即可，hover 联动留作后续 polish）。
- 不实现 teammate 消息的全文搜索（teammate body 已经在 `cdt-discover::search_extract` 中按 user 消息纳入 entry——但因为 chunk 层不再产 UserChunk，搜索命中跳转的"高亮锚点"机制留给后续 change 处理；本期搜索结果命中 teammate body 时跳到对应 AIChunk 即可，UI 不报错即视为可接受降级）。

## Decisions

### D1: teammate 消息落点 = AIChunk 子项（不是顶层 chunk）

**选择**：在 `AIChunk` 上新增 `teammate_messages: Vec<TeammateMessage>` 字段，所有 teammate user 消息在 chunk-building 阶段被注入到下一个 flush 出的 `AIChunk` 上。

**候选方案**：
- A（采纳）：嵌入 AIChunk —— teammate 与 SendMessage 因果绑定，UI 紧邻渲染。
- B：新顶层 `Chunk::Teammate` —— teammate 作为时间轴独立节点。

**理由**：
1. **多队友并行场景下 A 的因果对应清晰**：当一次 AI turn 里 SendMessage→3 队友，3 条 reply 在时间轴上独立呈现（B）会让用户依赖 reply-to chip 才能配对；A 直接把每条 reply 紧贴对应 SendMessage 渲染。
2. **token 归因更自然**：teammate reply 的 token 实际计入主 session（user 消息回流），属于该 turn 的 `metrics.input_tokens`；放进 AIChunk 内不破坏现有 metrics 语义。
3. **UI 类型扩散最小**：`Chunk` union 不变，前端 `chunk.kind` switch 不增分支，`Chunk` 序列化 / search-extract / chunk count 等外围逻辑零改动。
4. 与原版 TS 视觉/交互一致，复用现有视觉契约（左 3px 彩色边卡片、teammate badge、reply-to chip、噪声极简）。

**代价**：reply-to 配对状态需要在 chunk-building 跑一次，新增 `pending_teammates: Vec<PendingTeammate>` 缓冲区，在 `flush_buffer` 时与 buffer 内 SendMessage tool_use 配对后注入到产出的 AIChunk。

### D2: Reply-to 配对算法 —— 跨 turn 后向扫描已 emit AIChunks

**选择**：teammate 消息在 chunk-building 主循环里**不立即** flush，而是缓存到 `pending_teammates: Vec<PendingTeammate>`；下一次 flush 出 AIChunk 时，把 pending 内每条 teammate 按以下顺序配对 reply-to：

1. 优先在**新 flush 的 AIChunk** 自身的 `tool_executions` 中向后扫描，找 `tool_name == "SendMessage"` 且 input.recipient 匹配 teammate_id 的未配对 tool_use（已 used 的 tool_use_id 用本地 set 记录）。
2. 命中：`reply_to_tool_use_id = Some(...)`，把 teammate 注入该 AIChunk.teammate_messages。
3. 未命中：在**已 emit** 的最近一个 AIChunk（向前回溯，最多回溯 3 个 AIChunk）扫描同模式的未配对 tool_use。
4. 仍未命中：`reply_to_tool_use_id = None`，注入到当前 flush 的 AIChunk.teammate_messages（孤儿，UI 追加到 turn 末尾）。

**候选方案**：
- A（采纳）：上述跨 turn 后向扫描，配对失败保留为 orphan。
- B：仅在同 AIChunk 内配对，配对失败丢弃。

**理由**：
- 实测 JSONL 中 teammate reply 通常出现在触发 SendMessage 的 AIChunk 之后的下一条 user 消息位置（即上一个 AIChunk flush 后），所以下一个 AIChunk 接到的 teammate 实际属于上一个 AIChunk 的 SendMessage —— B 会全部错配为 orphan。
- 跨 turn 回溯上限 3 个 AIChunk 是兜底（罕见的"队友延迟回信"场景），避免无界扫描。
- "已配对"集合按 tool_use_id 去重，避免一条 SendMessage 被多条 teammate 抢配对。

**实现位置**：
- 新文件 `cdt-analyze/src/team/reply_link.rs`：纯函数 `link_teammate_to_send_message(pending: &TeammateAttrs, candidate_chunks: &[&AIChunk]) -> Option<String>`，独立单测。
- chunk builder 调用：在 `flush_buffer` 内、`AIChunk` 构造完成但未 push 到 `out` 之前，对每条 pending teammate 调用 `link_teammate_to_send_message(&teammate, &chain)`，其中 `chain = [last_few_in_out, &this_chunk_about_to_push]`。

### D3: AIChunk 字段扩展 + camelCase IPC

**选择**：在 `cdt-core::chunk::AIChunk` 新增字段：

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub teammate_messages: Vec<TeammateMessage>,
```

`TeammateMessage` 类型放 `cdt-core::chunk` 模块（与 `AIChunk` 共生命周期）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeammateMessage {
    pub uuid: String,
    pub teammate_id: String,
    pub color: Option<String>,
    pub summary: Option<String>,
    pub body: String,
    pub timestamp: DateTime<Utc>,
    pub reply_to_tool_use_id: Option<String>,
    pub token_count: Option<u64>,   // 来自 ParsedMessage.usage.input_tokens（teammate user 消息的 input usage 即"灌进主线的 token"）
    pub is_noise: bool,
    pub is_resend: bool,
}
```

**理由**：
- 字段全部 camelCase 序列化（前端契约）；`Vec` 用 `skip_serializing_if = "Vec::is_empty"` 减小默认 payload。
- `token_count: Option<u64>`：teammate 消息体本身在主 session 上的 token 估算，优先取 `ParsedMessage.usage.input_tokens` 的差值（无 usage 时退化到 body 字符数 ÷ 4 启发式）。
- `is_noise` / `is_resend` 在后端预算并落到字段上，前端不再重算（与"BaseItem token 稳定"同思路）。

**回滚开关**：`cdt-analyze::chunk::builder.rs` 顶部 `const EMBED_TEAMMATES: bool = true;`。`false` 时 chunk-building 退回旧 `continue` 行为，`AIChunk.teammate_messages` 永远空数组（被 `skip_serializing_if` 直接省略，前端兼容老 payload）。

### D4: 噪声 / Resend 检测 —— 后端预算落字段

**选择**：在 `cdt-analyze::team::noise.rs` 新建两个纯函数：

```rust
pub fn detect_noise(body: &str, teammate_id: &str) -> bool;
pub fn detect_resend(summary: Option<&str>, body: &str) -> bool;
```

`detect_noise` 与原版 TS 同算法：
- `teammate_id == "system"` 且 body 是 JSON 且 `type ∈ {idle_notification, shutdown_request, shutdown_approved, teammate_terminated}` → noise
- `teammate_id == "system"` 且 body trim 后长度 < 200 → noise
- 其它 teammate_id 但 body 是上述类型 JSON → noise

`detect_resend` 与原版同正则集：`/\bresend/i`、`/\bre-send/i`、`/\bsent\b.{0,20}\bearlier/i`、`/\balready\s+sent/i`、`/\bsent\s+in\s+my\s+previous/i`，命中 summary 或 body 前 300 字符任一即 true。

**理由**：
- 检测在后端预算便于将来在测试中固化行为（`insta` 快照），且避免前端 UI 重复实现 / 维护正则。
- 字段透明传给前端，前端按 `is_noise` 走极简单行 / 按 `is_resend` 加 RefreshCw 图标即可。

### D5: UI 落点 —— SessionDetail 按 timestamp 穿插渲染

**选择**（apply 阶段更正）：`displayItemBuilder.ts` 把所有 displayItems（thinking / text / tool / subagent / teammate_message / teammate_spawn）按各自 `timestamp` **稳定排序穿插**——同 timestamp 保留 push 顺序。slash 命令仍排最前（与 AI turn 整体绑定）。

**Reply-to 字段不决定渲染位置**——`replyToToolUseId` 仅用于 teammate 卡片 header 的 chip 文本（"↪ reply"），位置由 `tm.timestamp` 决定。这样对齐原版 `displayItemBuilder.ts::sortDisplayItemsChronologically`，且兼容"teammate 主动发言无 SendMessage 配对"场景（不会全部堆在 turn 末尾）。

**propose 阶段曾选择"按 reply_to 紧贴 SendMessage"**——apply 时实测发现：
- 不少 team 流程不走 `SendMessage`（spawn 阶段走 `Agent` tool，回信由 teammate 主动发起）
- reply_to 配对失败导致大量 orphan，全部追加到 turn 末尾，时序错乱

故改回原版的 timestamp 排序方案。`reply_to_tool_use_id` 保留为字段，仅作 chip 显示，不变。

**前端类型扩展**（`ui/src/lib/api.ts`）：
```typescript
export interface TeammateMessage {
  uuid: string;
  teammateId: string;
  color: string | null;
  summary: string | null;
  body: string;
  timestamp: string;
  replyToToolUseId: string | null;
  tokenCount: number | null;
  isNoise: boolean;
  isResend: boolean;
}
export interface AIChunk {
  // 新增
  teammateMessages?: TeammateMessage[];
  // 既有字段不变
}
```

**新组件**：`ui/src/components/TeammateMessageItem.svelte` —— 单文件实现 noise / resend / normal 三态渲染，复用现有 BaseItem 视觉令牌（`CARD_BG / CARD_BORDER / CARD_HEADER_BG / CARD_ICON_MUTED / CARD_TEXT_LIGHT` 在 `app.css` 中已存在）。

**lazyMarkdown 扩展**：`ui/src/lib/lazyMarkdown.svelte.ts` 的 `Kind` union 加 `"teammate"`；teammate body 走与 user / ai 同样的视口懒渲染管线（避免大批 teammate body 一次性渲染）。

**team color 映射**：复用原版 TS 的 `getTeamColorSet(color: string)` 算法（14 色调色板 + 模糊匹配），作为新文件 `ui/src/lib/teamColors.ts`。color 缺失时退化为 `--color-text-muted` + `--color-border`。

### D5b: 多 block teammate 解析 + Teammate spawned 极简卡片（apply 阶段补）

**多 block 解析**：原版 `parseAllTeammateMessages` 用 global regex 把一条 user 消息含 N 个 `<teammate-message>` 块各自解出。`cdt-analyze::team::detection::parse_teammate_attrs` 旧实现取"首个 `>` 之后到末尾再剥最后的 `</teammate-message>`"——多 block 时会把所有块串到一个 body，丢失后续块。修法：新增 `parse_all_teammate_attrs(msg) -> Vec<TeammateAttrs>` 用全局 regex 解析；chunk builder 改用 `extend(build_pending_teammates(msg))` 一次产 N 条。多 block 时各条 uuid 加 `-{idx}` 后缀去重。

**Teammate spawned 极简卡片**：原版 `LinkedToolItem.tsx::isTeammateSpawned` 检测 `tool_result.toolUseResult.status === "teammate_spawned"`，不渲染普通 tool item，而是输出"圆点 + member-X badge + Teammate spawned"极简单行。修法：
- `cdt-core::ToolExecution` 加 `teammate_spawn: Option<TeammateSpawnInfo>` 字段（含 `name` + `color`），`#[serde(skip_serializing_if = "Option::is_none")]`
- `cdt-analyze::tool_linking::pair::extract_teammate_spawn` 在 pair 阶段从 user 消息顶层 `toolUseResult` 抽出 status / name / color
- 前端 `displayItemBuilder.ts` 检测到 `exec.teammateSpawn` 非空时把整条 tool item 替换为 `{ type: "teammate_spawn" }` DisplayItem
- `SessionDetail.svelte` switch 加 `teammate_spawn` 分支渲染极简单行

### D6: Sidebar 标题修复

**选择**：在 `cdt-api::session_metadata::sanitize_for_title` 同文件加：

1. 标签白名单数组追加 `"teammate-message"`，整段 XML 被剥除。
2. 新增 fast-path：在剥除之前先检测 `text.starts_with("<teammate-message")`，若整段被一个 teammate-message 包裹（无其它显著内容），优先取 `summary=` 属性内容作为标题候选；`summary` 为空时再走 fallback 把整段剥掉，让 title 最终落到 `command_fallback` 或保持 `None`。

**理由**：
- summary 是队友自己写的"主题"，比 body 前 100 字更具信息量。
- `<teammate-message>` 单独剥光会让大量 team session 标题变空 → 退到 `None` → sidebar fallback 到 sessionId 前 8 字符——可接受但 summary 优先更好。

### D7: 测试策略

- `cdt-analyze::team::reply_link::tests`：纯 Rust 单测 8 个场景（A 配对 A / 跨 turn 配对 / 多队友并行 / orphan / 同名 teammate 多回复 / 跨 SendMessage 不抢配对 / 回溯上限 / EMBED_TEAMMATES=false 回退）。
- `cdt-analyze::team::noise::tests`：JSON noise / 长 body 不算 noise / system 短 body 算 noise / resend 五种正则覆盖。
- `cdt-analyze::chunk::builder::tests`：teammate 消息不产 UserChunk + 嵌入下一个 AIChunk + orphan 注入当前 AIChunk + EMBED=false 回退。
- `cdt-api::session_metadata::tests`：sanitize_for_title 含 teammate-message 时取 summary / 无 summary 退回 None。
- `cdt-api::tests::get_session_detail_with_teammate.rs`（新建集成测）：fixture JSONL 含 SendMessage→teammate-reply 流，断言 IPC 输出 `teammateMessages` 字段结构 / replyToToolUseId 配对 / camelCase。

## Risks / Trade-offs

- **[chunk 测试快照大面积重写]** → 现有 `cdt-analyze` 的 insta 快照可能因 `AIChunk` 多字段而 diff 爆炸。Mitigation：`teammate_messages` 用 `skip_serializing_if = Vec::is_empty`，无 teammate 的 fixture 输出不变；`INSTA_UPDATE=always` 重跑只更新真有 teammate 的少数 fixture。
- **[reply-to 跨 turn 回溯误配对]** → 极端场景下队友 A 在两个连续 turn 都收到 SendMessage（重复任务），后到的 reply 可能被错配到第一次 SendMessage。Mitigation：跨 turn 回溯上限 3 个 AIChunk + per-AIChunk 已配对 set 去重，超出回溯窗口的 reply 走 orphan 路径；UI orphan 渲染本身不丢信息，仅丢"reply-to chip 的精确指向"。
- **[noise 误判过滤掉真实短消息]** → `teammate_id == "system"` 且 body < 200 字符全部当 noise 可能误杀真 system 通告。Mitigation：与原版 TS 完全同算法，原版长期跑无明显投诉；如发现误杀只需在 `noise.rs::detect_noise` 调阈值。
- **[token_count 估算不准]** → teammate body 在主 session 的实际 token 取决于上下文 cache 状态。Mitigation：优先取 `usage.input_tokens` 真实值，缺失时启发式估算；UI 显示 `~` 前缀表明是估算值。
- **[search-extract 命中 teammate body 后跳转锚点失效]** → teammate 不再产 UserChunk，按 uuid 跳转的 search 结果可能找不到 chunk。Mitigation：search 结果命中 teammate uuid 时跳到包含该 teammate 的 AIChunk（按 chunk-building 阶段建 `teammate_uuid → containing_ai_chunk_index` 反查表，本期改动可不实现，留作后续 polish；UI 上 search 不报错即可）。

## Migration Plan

1. **回滚单切**：`cdt-analyze::chunk::builder.rs` 顶部 `const EMBED_TEAMMATES: bool = true;` 改 false 即回退旧行为。`AIChunk.teammate_messages` 永远空数组，前端 `if (chunk.teammateMessages?.length)` 守卫保证渲染兼容。
2. **HTTP API 自动适用**：`http-data-api` 路径与 `LocalDataApi` 共用 `build_chunks`，无须额外改动。
3. **快照接受**：本 change 实施时跑 `INSTA_UPDATE=always cargo test -p cdt-analyze`，diff 仅在新增的 teammate fixture 上出现。
4. **回归监测**：`cdt-api::tests::perf_get_session_detail`（perf 基准）含 teammate 大会话时跑一次 verify，确认 IPC payload 增量在可接受范围（teammate body 走默认序列化，无新 OMIT 链路）。
5. **CLAUDE.md 更新**：本 change archive 后顺手把"原版 UI 参考 / port 状态判定"段落补一句"teammate 渲染对齐原版 `TeammateMessageItem.tsx`"。

## Open Questions

- 是否需要把 teammate body 也走 IPC OMIT 瘦身？当前估算单条 teammate body 大概数百到数千字符（chat 级），即使 session 含 50 条 teammate 总量约几十 KB，相对 tool_output / image 体量小一个数量级。**当前决策**：本期不 OMIT；后续如出现 teammate-heavy session 性能回归，按现有 OMIT 模式补 `OMIT_TEAMMATE_BODY` 一行开关 + `get_teammate_body_lazy` IPC。
- reply-to chip 的 hover→ 高亮 SendMessage 反向联动是否本期实现？**当前决策**：本期 chip 仅展示文案，不实现 hover 联动（原版有 `onReplyHover` 回调 + spotlight 类）；后续 polish change 内补。

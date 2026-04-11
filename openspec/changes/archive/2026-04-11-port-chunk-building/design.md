## Context

`cdt-parse` 已经产出 `ParsedMessage` 序列。chunk-building 的职责是把该序列按语义分组成独立的 `Chunk`，供 UI、统计、搜索等下游消费。TS 原版在 `src/main/services/analysis/ChunkFactory.ts` 与 `ChunkBuilder.ts` 中实现，核心状态机大致如下：

1. 顺序扫描消息，跳过 `isSidechain`；
2. 根据 `MessageCategory` 分派到四种 chunk 构造路径；
3. 连续 assistant 消息落入同一个 `AIChunk.responses` 缓冲；
4. 遇到真实用户消息 / system 消息 / compact summary 时 flush 当前 buffer；
5. AIChunk 内部走一次 SemanticStep 抽取，再叠加 tool linking 结果。

Rust port 只做步骤 1–5 里的结构部分，不做 tool 链接 / subagent 归集（下一次 port）。

## Goals / Non-Goals

**Goals:**
- 在 `cdt-core` 定义不依赖 tokio 的 `Chunk` 及子类型，保证可序列化（serde）。
- 在 `cdt-analyze` 提供 `build_chunks` 纯函数，输入 `&[ParsedMessage]`，输出 `Vec<Chunk>`，全同步、可被任意 runtime 调用。
- 覆盖 spec 中属于 chunk-building 自身职责的 scenario：独立 chunk、assistant coalescing、sidechain 过滤、hard-noise 过滤、metrics 汇总、compact summary → CompactChunk、语义步骤按序抽取。
- 用 `insta` 做一组端到端快照，锁定 chunk 顺序与关键字段。

**Non-Goals:**
- 不实现 `tool_use` ↔ `tool_result` 匹配（留给 `port-tool-execution-linking`）。
- 不实现 Task 过滤（需要 subagent 信息，同上）。
- 不实现 subagent Process 归集（留给 `port-team-coordination-metadata`）。
- 不引入成本估算价格表；`ChunkMetrics::cost_usd` 暂为 `None`。
- 不做分组 UI 的 `SemanticStepGrouper`（TS 里是 UI 细节，spec 明确不约束）。

## Decisions

### D1：`Chunk` 放在 `cdt-core`，`build_chunks` 放在 `cdt-analyze`
- 理由：多个下游 crate（`cdt-api`、将来的 UI）都要引用 `Chunk` 类型本身，而构造逻辑只在分析层用。遵守 `cdt-core` 不依赖 tokio 的规则。
- 备选：全部放在 `cdt-analyze`，其它 crate 再 re-export——rejected，违反"共享类型在 core"的约定。

### D2：`Chunk` 用 enum + 公共头结构
```rust
pub enum Chunk {
    User(UserChunk),
    Ai(AIChunk),
    System(SystemChunk),
    Compact(CompactChunk),
}
```
公共字段（`timestamp`、`duration`、`metrics`）下放到每个变体结构体自身，不再抽一层 `ChunkHeader`，保持 match 简洁。duration 用 `chrono::Duration`，允许 `None`（单条消息没有区间）。

### D3：assistant 合并策略
buffer = `Vec<AssistantResponse>`；遇到以下任一条件时 flush：
1. 下一条是真实 user（非 tool_result-only 包装）；
2. 下一条是 SystemChunk 对应的 `local-command-stdout` 消息；
3. 下一条是 compact summary；
4. 到达输入末尾。

真实用户消息判断：`MessageCategory::User` 且 `content` 不是"仅 tool_result 包裹"。tool_result-only 用户消息会被合并进上一个 `AIChunk.responses` 的最后一条 assistant 响应的 `tool_results` 字段（给未来 tool linking 留 hook）。

### D4：SemanticStep 占位
```rust
pub enum SemanticStep {
    Thinking { text: String, timestamp: DateTime<Utc> },
    Text { text: String, timestamp: DateTime<Utc> },
    ToolExecution { tool_use_id: String, tool_name: String, timestamp: DateTime<Utc> },
    SubagentSpawn { placeholder_id: String, timestamp: DateTime<Utc> },
}
```
`ToolExecution` / `SubagentSpawn` 只携带 id/name，等后续 port 把它指向真实 `ToolExecution` / `Process`。本 port 里 subagent 识别推迟，因此 SubagentSpawn 不会被实际产出，但枚举变体先占位，避免下次 port 改动 `cdt-core` 公共类型。

### D5：metrics 汇总
`ChunkMetrics { input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, tool_count, cost_usd: Option<f64> }`。
- token：累加 `response.usage`；
- tool_count：遍历 `AIChunk.responses` 的 `tool_calls.len()` 求和，本次不考虑 Task 过滤（因此 tool_count 暂时包含 Task 调用，待 tool-execution-linking port 时在那个 change 里减去，spec delta 会说明）；
- cost_usd：`None`。

### D6：CompactChunk 触发源
使用 `ParsedMessage.is_compact_summary == true` 作为唯一来源。TS 里靠 display item `type === 'compact'`，行为等价（followups.md 已记录）。每遇到一条 `is_compact_summary` 消息就 flush 当前 buffer 并产出 `CompactChunk { summary_text, timestamp }`。

### D7：SystemChunk 识别
判定条件：`MessageCategory::System`，或 user 消息 content 精确被 `<local-command-stdout>...</local-command-stdout>` 包裹。解析器已经把空 stdout / stderr 归入 `HardNoise`，因此 SystemChunk 只需处理非空的 stdout。

### D8：纯函数 + snapshot 测试
- `build_chunks(&[ParsedMessage]) -> Vec<Chunk>` 不接触 I/O、不需要 async。
- 单元测试覆盖每个 scenario；集成测试 `crates/cdt-analyze/tests/chunks.rs` 用 `insta::assert_debug_snapshot!` 锁定 3 份 fixture（user+ai、多轮 ai coalescing、compact 边界）。
- fixture 放 `crates/cdt-parse/tests/fixtures/` 下现有文件，或复制一份到 `cdt-analyze/tests/fixtures/` 以避免跨 crate 路径耦合——**选后者**。

## Risks / Trade-offs

- **[Risk] tool_count 在本 port 里包含 Task 调用，和下一步 port 的 Task 过滤语义冲突** → Mitigation：spec delta 明确"tool_count 在 tool-execution-linking port 完成前统计所有 tool_use，之后按 Task 过滤语义修正"，并在 tasks.md 记录下一个 change 必须同步更新 snapshot。
- **[Risk] SemanticStep 占位类型与未来 tool-execution-linking 的 `ToolExecution` 结构不兼容** → Mitigation：只暴露 id+name+timestamp 字段，不内嵌结构体；后续 port 在 `SemanticStep::ToolExecution` 里新增可选字段，不破坏现有 match。
- **[Trade-off] 把 `Chunk` 放 core 会让 core 的公开 API 稍大** → 但所有字段都是纯数据，没有行为耦合，符合 core 的定位。
- **[Risk] tool_result-only 的 user 消息如何归属**：若决定并入上一条 assistant response，会在没有 tool_use 的边角 case 里产生 orphan tool_result 字段 → Mitigation：若上一条不是 AIChunk，则该 user 消息走普通 UserChunk 路径，orphan 行为在 `port-tool-execution-linking` 里统一处理。

## Migration Plan

本 port 是纯新增：`cdt-analyze` 之前只有空 `lib.rs`，不存在向后兼容问题。步骤：
1. 在 `cdt-core` 新增 `chunk` 模块并在 `lib.rs` re-export。
2. 在 `cdt-analyze` 新增 `chunk` 模块，补 `Cargo.toml` 依赖。
3. 所有测试通过后，在 `openspec/changes/port-chunk-building/specs/chunk-building/spec.md` 写 delta，`opsx:archive` 时合并到正式 spec。

回滚：直接 revert 该 change 的 commit；`cdt-analyze` 回退到空 `lib.rs` 即可，无外部消费者。

## Open Questions

- Q1：duration 字段对 UserChunk 是否总是 `None`？建议是——UserChunk 只承载一条消息，没有区间。决策：**是**，写进 spec scenario。
- Q2：`AssistantResponse` 是否要保留原始 `ParsedMessage` 引用？建议保留 `uuid` 与 `timestamp`，其它字段按需复制，避免 chunk 持有长生命周期引用。决策：**是**，用 `String` uuid + cloned 字段。

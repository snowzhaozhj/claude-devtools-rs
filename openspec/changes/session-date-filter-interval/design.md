## Context

`SessionSummary.timestamp` = 文件 mtime，`QueryFilter` 的 `--since/--until` 用 `s.timestamp >= since && s.timestamp <= until` 做单点匹配。用户深夜活跃的会话 JSONL 文件最后写入跨过午夜，mtime 落在"第二天"，导致 `--since yesterday --until today` 系统性漏掉晚间会话。

当前数据链路：`fs::metadata().modified()` → `FsMetadata.mtime` → `SessionStat.mtime_ms` → `Session.last_modified` → `SessionSummary.timestamp` → `QueryFilter.apply()`。全链只有 mtime 一个时间维度。

## Goals / Non-Goals

**Goals:**
- 让 `--since/--until` 过滤匹配"在该时间段内有活动的会话"（区间交集语义）
- 零额外 I/O：`created` 从已有 `stat` 调用的 `metadata.created()` 取
- 向后兼容：新增字段用 `#[serde(default)]`，不改 `timestamp` 语义

**Non-Goals:**
- 不改 `timestamp` 字段含义（仍为 mtime，用于排序/展示/sidebar 列表排序）
- 不做精确到消息级别的活动时间判定（那需要扫文件内容）
- 不改 `stats` 命令的 session inclusion 和 `session_hour` 统计逻辑——stats 的 since 过滤（`cdt-cli/src/main.rs` 和 `mcp/mod.rs` 的 `get_stats`）仍保持 mtime 单点语义，独立于本次改动
- 不改前端排序/展示逻辑

## Decisions

### D1: 用文件 birthtime 近似 session 创建时间

**候选**：
- (a) 读 JSONL 第一行的 `timestamp` 字段 — 精确但需要打开每个文件读一行
- (b) 用 `fs::metadata().created()` — 零 I/O，从已有 stat 取

**选择 (b)**。理由：session 目录可能有几千个文件，逐个打开读首行的成本不可接受（违反 perf.md 预算）。文件 birthtime 与 session 首条消息时间在实践中一致（Claude Code 创建 session 时新建 JSONL 文件）。极少数场景（文件被 cp/rsync 复制）会丢失 birthtime，此时 fallback 到 mtime 退化为当前行为。

### D2: `created` 不可用时 fallback 到 mtime

`std::fs::Metadata::created()` 在 Linux ext2/ext3、某些网络文件系统上返回 `Err`。fallback 策略：`metadata.created().unwrap_or(mtime)`。效果：不支持 birthtime 的平台行为与改动前完全一致。

### D2b: `created > mtime` 反向区间归一化（codex F2）

文件被 cp/rsync 复制等场景可能导致 `created > mtime`（birthtime 是复制时间，但 mtime 保留了原文件的修改时间）。`created_ms()` SHALL 返回 `min(created, mtime)` 做归一化，确保 `[created, mtime]` 区间始终是正向区间。不归一化的反向区间会导致区间交集判定错误——`created <= until AND mtime >= since` 在 `created > mtime` 时可能误排除合法 session。

### D3: 过滤语义为区间交集

session 时间区间 = `[created, mtime]`，查询区间 = `[since, until]`。交集非空条件：`session.created <= until AND session.mtime >= since`。

当前 `filter.rs` 的 since 条件 `s.timestamp >= since` 已经是 `mtime >= since`（正确的一半）；only until 条件需要从 `s.timestamp <= until` 改为 `s.created <= until`。

### D4: `SessionSummary.timestamp` 语义不变

`timestamp` 仍然 = mtime，继续用于：
- sidebar 列表排序（最近修改的在前）
- 前端 `formatTime(session.timestamp)` 展示
- `session-metadata-update` SSE event 的排序依据
- `get_worktree_sessions` k-way merge 排序

新增 `created` 字段仅用于过滤，不参与排序和展示。

### D5: group 级剪枝适配

`engine.rs::list_sessions_cross_project` 当前用 `group.most_recent_session < since` 剪枝跳过整个 group。区间交集语义下此剪枝仍然正确——如果一个 group 内所有 session 的 mtime 都 < since，那它们的 `[created, mtime]` 区间不可能与 `[since, ...]` 有交集。无需改动。

### D6: IPC contract test 同步

`cdt-api/tests/ipc_contract.rs` 的 `SessionSummary` round-trip 测试需要加 `created` 字段验证。

## Risks / Trade-offs

- **精度 vs 性能**：文件 birthtime 不如 JSONL 首条消息 timestamp 精确，但避免了 N 次文件 I/O。对于过滤场景（粒度是"天"）误差可忽略。
- **平台一致性**：Linux 旧内核/旧文件系统不支持 birthtime 时退化为当前行为。不会更差，但也不会改善。用户群体以 macOS 为主，影响有限。
- **`0954c8bb` 类误包含**：文件被 session 外因素修改导致 mtime 偏移的情况（罕见），区间交集会把这类 session 包含进来。可以接受——比漏掉 47% 的会话好得多。

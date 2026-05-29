## Context

`get_session_detail` 调用 `scan_subagent_candidates_for_detail` 扫描当前会话的所有 subagent。当前实现（`local.rs:5071-5414`）有两个叠加瓶颈：

1. **`parse_subagent_candidate`** 对每个 subagent 文件做两次完整读取：第一次用 `serde_json::from_str::<Value>` 逐行扫全量拿 `end_ts`（最后一行 timestamp），第二次 `parse_file_via_fs` 结构化解析所有行。31 个 subagent × 每个 ~30ms = 930ms。
2. **同一 project 内串行**：跨 project_dir 有 `Semaphore(8)` 并发，但命中 subagent 的 project 内部是 `for sub_entry in sub_entries` 串行循环。31 个 subagent 通常全在同一 project 下。

当前 user/real = 0.17（CPU 实际工作 ~158ms，其余等 I/O），说明并行化空间巨大。

## Goals / Non-Goals

**Goals:**
- cold path `scan_subagents_ms` 从 ~930ms 降到 < 200ms
- 维持 user/real ≤ 0.66（Sem(4) 限流，不让辅助工具短时打满 CPU）
- 零行为变更：resolve 结果、IPC payload、前端渲染完全不变

**Non-Goals:**
- 不加内存缓存（避免常驻 RSS 增长；首次 < 200ms 已满足预算）
- 不改 `OMIT_SUBAGENT_MESSAGES` 策略
- 不改 `CROSS_PROJECT_SUBAGENT_SCAN` 跨目录扫描逻辑
- 不改 `resolve_subagents` / `build_chunks_with_subagents` 的输入契约

## Decisions

### D1：合并双重文件读取为单次结构化 parse

**选择**：删除 `parse_subagent_candidate` 中的第一阶段（`Value` 泛解析全行扫描），改为直接调 `parse_file_via_fs` 一次性获得 `Vec<ParsedMessage>`，再从中提取 metadata。

**理由**：
- 第一阶段存在的目的是提取 `spawn_ts` / `end_ts` / `parent_session_id` / `description_hint` 四个字段 + 判断 warmup。这些信息全部可从 `ParsedMessage` 的结构化字段中获取。
- `ParsedMessage` 已有 `timestamp`（取首尾即得 spawn_ts / end_ts）、`parent_uuid`（= parent_session_id）、`content` + `message_type`（判 warmup + 提取 description_hint）。
- 省掉一次 fd open + 一次全量 `Value::from_str` 泛解析（`Value` 比 typed struct 多 ~40% 分配开销）。

**替代方案（已否决）**：
- "只优化第一阶段（seek 尾部 4KB 提取 end_ts）"——合并后第一阶段本身就不存在了，seek 优化无从施加。
- "保留双阶段但并行两次读取"——两次读同一文件并行没有 I/O 收益（同一磁盘 sector），反而增加 fd 开销。

### D2：同项目内 subagent 并行化 Semaphore(4)

**选择**：在 `scan_subagent_candidates_cross_project` 内层（同一 project 的 sub_entries 循环）改为 `futures::future::join_all` + 独立 `Semaphore(4)` 限流。

**理由**：
- 31 个 subagent 串行 × 每个 ~15ms（合并读取后单次 parse 预计 ~15ms）= 465ms → 并行 Sem(4) ≈ 8 批 × 15ms = 120ms
- Sem(4) 保证 user/real ≈ 158ms / 240ms = 0.66（低于 1.0 红线）
- 并发 fd = 4 × 1 = 4（合并后单次读取只需 1 fd），远低于 ulimit

**替代方案（已否决）**：
- Sem(8)：user/real ≈ 1.3，120ms > 50ms burst 豁免线，会被 perf.md 拒
- Sem(2)：过保守，wall ≈ 240ms 但仍可接受；选 4 是 wall/CPU 最优平衡点

**实现细节**：内层 semaphore 与外层 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 独立——外层控制跨 project 并发，内层控制同 project 内 subagent 并发。两层 permit 乘积最大 = 8 × 4 = 32 并发 task，但实际只有 1 个 project 命中 subagent（其余 read_dir 返回 ENOENT 立即释放外层 permit），所以稳态并发数 = 4。

### D3：metadata 提取从 `ParsedMessage` 流中 O(1) 头尾取

**选择**：parse 完成后 `msgs.first()` 拿 spawn_ts + parent_uuid，`msgs.last()` 拿 end_ts，前 10 条扫 description_hint + warmup 判定。

**理由**：与原实现语义完全等价——原版就是"前 10 行提取 metadata + 全量扫描拿最后一个 timestamp"。结构化 parse 后 messages 已按行序排列，首尾操作是 O(1)。

## Risks / Trade-offs

**[R1] 并行化后 CPU burst 接近 perf 边界** → 用 Sem(4) 而非 8 限流；burst 持续时间 < 120ms << 200ms 交互峰值；辅助工具定位可接受。

**[R2] `parse_file_via_fs` 比原版 `Value` 扫描可能慢** → 实测 `parse_entry_at`（typed struct）比 `Value::from_str`（动态分配）快 ~30-40%（typed 跳过无关字段）。合并后单次 parse 总耗时 ≤ 原双重读取的 60%。

**[R3] 旧结构兜底路径也需同步改** → `scan_subagent_candidates_cross_project` 的 L5146-5169（legacy fallback）也串行调 `parse_subagent_candidate`，需同步受益于 D1（合并读取），但不需要并行化（legacy 路径候选数通常 < 5）。

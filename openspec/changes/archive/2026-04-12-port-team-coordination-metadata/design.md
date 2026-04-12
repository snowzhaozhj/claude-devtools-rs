## Context

`cdt-analyze` 已有 chunk-building、tool-execution-linking、context-tracking 三个 capability 完整实现。第四个 `team-coordination-metadata` 是空 module。`cdt-core::process` 定义了 `TeamMeta { team_name, member_name, member_color }` 和 `Process.team: Option<TeamMeta>`。

前序 port 留了两个显式 TODO：
1. `filter_resolved_tasks` 纯函数已实现，但 `build_chunks` 未调用（`tool_linking/filter.rs` 注释）
2. context aggregator 的 `teammate_message` display item 留空（`aggregator.rs:113`）

## Goals / Non-Goals

**Goals:**
- `is_teammate_message` + attribute 解析
- 7 种 team 工具摘要格式
- `extract_team_meta_from_task`：从 Task input 提取 team info → `TeamMeta`
- `build_chunks` 接入 `filter_resolved_tasks` + teammate guard
- context aggregator 补 `teammate_message` 路径
- 测试覆盖 spec 的 6 个 scenario

**Non-Goals:**
- `enrichTeamColors` 多次扫描（TS 侧的二次过程）→ 简化为从 Task input 一次提取
- `propagateTeamMetadata` 跨文件传播 → 调用方职责
- `SubagentSpawn` semantic step 变体 → chunk-building 级别改动，scope 内简单标记

## Decisions

### D1: Module 结构

```
cdt-analyze/src/team/
├── mod.rs               # pub use
├── detection.rs         # is_teammate_message + parse_teammate_attrs
├── summary.rs           # format_team_tool_summary (7 tools)
└── enrichment.rs        # extract_team_meta_from_task
```

### D2: Teammate 检测方式

用 `regex::Regex` 匹配 `^<teammate-message\s+teammate_id="([^"]+)"` + 可选 `color="([^"]+)"` + `summary="([^"]+)"`。`LazyLock` 编译一次。

### D3: `filter_resolved_tasks` 接入点

在 `build_chunks` 开头，`pair_tool_executions` 之后、消息遍历之前，调用 `resolve_subagents` + `filter_resolved_tasks`。但 `resolve_subagents` 需要 `SubagentCandidate` 列表——这个列表的装载（从磁盘扫描 subagent session）不属于本 capability。

**方案**：`build_chunks` 签名不变，内部不调用 `resolve_subagents`（因为没有 candidates）。改为提供 `build_chunks_with_subagents(messages, candidates)` 扩展入口，调用方在有 candidates 时使用。原 `build_chunks` 保持现有行为。

### D4: Teammate guard 在 `build_chunks`

在处理 `MessageCategory::User` 分支开头增加 `if team::is_teammate_message(msg) { continue; }`，跳过 teammate 消息不产出 `UserChunk`。

## Risks / Trade-offs

- **[Trade-off] `build_chunks_with_subagents` 增加 API surface** → 但保持了向后兼容，现有调用方不受影响
- **[Risk] regex 依赖** → `cdt-analyze` 新增 `regex`，但 workspace 已有，不增加编译图复杂度

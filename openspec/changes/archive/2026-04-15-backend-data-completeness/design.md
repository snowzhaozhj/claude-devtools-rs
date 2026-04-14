## Context

数据层 13 个 capability 全部完成，但 API 层有三个数据缺口阻碍前端显示完整信息：

1. **Subagent 数据为空**：`resolve_subagents` 已实现但 `get_session_detail` 未调用
2. **Slash 命令丢失**：`build_chunks` 过滤 isMeta 消息时丢弃了 slash 命令信息
3. **Search 是 stub**：`LocalDataApi.search()` 返回空数组，未对接已完成的 `SessionSearcher`

## Goals / Non-Goals

**Goals:**
- `get_session_detail` 返回的 `AIChunk.subagents` 包含真实 subagent 数据
- Slash 命令信息从 isMeta 消息中提取并可用于前端 summary 显示
- `search()` 调用 `SessionSearcher` 返回真实搜索结果；Tauri 层暴露 `search_sessions` command

**Non-Goals:**
- 前端全局搜索 UI（本批只暴露 API，UI 留给后续批次）
- Team coordination metadata 的完整集成（颜色、层级关系等——已有 TODO 标注）
- Slash 命令的详细 UI 渲染（本批只做 summary 计数）

## Decisions

### D1：Subagent 候选获取方式

**选择**：在 `LocalDataApi` 中利用已有的 `ProjectDiscovery` 扫描同 project 下的 session 列表，构建 `SubagentCandidate` 列表传给 `resolve_subagents`。

**备选**：
- 让 `build_chunks` 接受 candidates 参数 → 需要改 `build_chunks` 签名，影响所有调用点
- 在 `cdt-analyze` 层做 IO 获取候选 → 违反 cdt-analyze 纯同步原则

**理由**：`cdt-api` 是 facade 层，组装跨 crate 数据是它的职责。`resolve_subagents` 是纯函数，只需要准备好输入。

### D2：SubagentCandidate 构建

需要为每个候选 session 提取 `session_id`、`spawn_ts`、`metrics`、`description`。方案：

1. 从 `get_session_detail` 的 session 所在 project 获取 session 列表（已有 `list_sessions`）
2. 对每个候选 session 文件做轻量扫描（只读前几行获取时间戳和 parent 信息）
3. `cdt-discover` 已有 `scan_session_metadata` 可复用

### D3：Slash 命令提取位置

**选择**：在 `cdt-analyze` 的 `build_chunks` 中，遇到 isMeta 消息时提取 slash 信息并记录到一个独立的 `Vec<SlashCommand>` 返回值中，由 API 层附加到对应 chunk。

**备选**：
- 在 `build_chunks` 内部直接修改 AIChunk 结构 → 需要扩展 AIChunk 类型，但 slash 信息不属于 AI 响应
- 在 API 层做二次扫描 → 需要重新解析消息，重复劳动

**理由**：Slash 命令从 isMeta user 消息的 `<command-name>` XML 标签中提取（格式：`<command-name>/xxx</command-name>`），这是解析阶段的工作。提取结果附加到紧随其后的 AIChunk，由 `build_chunks` 返回额外的 slash 列表。

### D4：Slash 数据结构

```rust
pub struct SlashCommand {
    pub name: String,           // e.g. "commit", "claude-hud:setup"
    pub message: Option<String>, // <command-message> 内容
    pub args: Option<String>,    // <command-args> 内容
    pub message_uuid: String,    // 所属 isMeta 消息的 uuid
    pub timestamp: DateTime<Utc>,
}
```

在 `AIChunk` 中新增 `slash_commands: Vec<SlashCommand>` 字段。前端 `buildAiGroupSummary` 按 `slash_commands.length` 计数显示。

### D5：Search 集成方式

**选择**：`LocalDataApi` 构造时接收 `SessionSearcher`（通过泛型或 Arc），`search()` 直接委托。

**理由**：`SessionSearcher` 已有完整的缓存和分阶段搜索实现，直接复用。Tauri 层新增 `search_sessions` command 透传即可。

## Risks / Trade-offs

- **[性能]** Subagent 候选扫描在大 project 下可能较慢 → 缓解：只扫描与当前 session 时间窗重叠的候选；SessionSearcher 已有 LRU 缓存
- **[类型扩展]** `AIChunk` 新增 `slash_commands` 字段会改变序列化格式 → 缓解：新字段默认空数组，前端已有 `?.length ?? 0` 防御
- **[build_chunks 签名]** 返回值从 `Vec<Chunk>` 变为包含 slash 信息的结构 → 缓解：用新函数 `build_chunks_full` 返回 `BuildResult { chunks, slash_commands }`，保留 `build_chunks` 兼容

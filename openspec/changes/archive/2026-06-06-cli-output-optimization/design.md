## Context

CLI (`cdt`) 和 MCP server 共处同一 binary (`cdt-cli` crate)。MCP 已有完善的 view 层（`ChunkEnvelope` + `ContentMode::Omit/Full` + `summarize_input` + 分页 + `grep_hit` 标记），CLI 直接 `serde_json::to_string_pretty()` 序列化 domain objects，没有 view 层。两条路径共享 `QueryEngine` 和 `DataApi`，但 MCP 在其上封装了一层视图转换，CLI 没有。

MCP view 层 17 个组件（struct/enum/fn）经审计确认零 MCP 协议依赖，只需 `cdt-core` + `serde` + `serde_json` + `chrono`，可干净提取。

CLI 和 MCP 还有 grep 语义差异：MCP 是 `kind_filter → grep → range/tail`，CLI 是 `kind_filter + range/tail → grep`（先窗口后 grep）。

当前没有任何测试验证 CLI JSON 输出结构，改动风险可控。

## Goals / Non-Goals

**Goals:**
- CLI 和 MCP 共享 view 层（`ChunkView`/`ToolExecView`/`ContentMode`/`summarize_input`），消除代码分叉
- CLI `sessions detail` 支持 `--content omit|full`，让 agent 获取 chunk 结构概览（~500B/chunk vs ~200KB/chunk）
- 修正格式契约缺陷（jsonl 假行为、exit(2)、truncate 不一致、中文对齐）
- 统一 CLI/MCP 的 grep 应用顺序
- 提供 `--json <fields>` 字段选择，让 agent 精确取字段
- 提升 table 模式人可读性（PATH 缩写、终端宽度自适应）

**Non-Goals:**
- 不改变 DataApi 层的数据加载策略（`ContentMode` 只影响序列化输出，不影响内存）
- 不修改 HTTP routes（`cdt-api/src/http/routes.rs`），HTTP 路径保持 raw domain object 输出
- 不改变 `--format json` 的默认输出结构（不指定 `--content` 时保持原有 raw `SessionDetail` JSON）
- 不引入 table 渲染库
- 不改变默认输出格式（保持 `--format table` 默认，不做 tty auto-detect 切换格式）

## Decisions

### D1: view 层放在 `cdt-cli::view` 模块，不新建 crate

从 `mcp/mod.rs` 提取到 `crates/cdt-cli/src/view.rs`，`main.rs` 和 `lib.rs` 同时声明 `mod view` / `pub mod view`。

**为什么不新建 `cdt-view` crate**：当前只有 CLI 和 MCP 需要（都在 `cdt-cli` 内），HTTP routes 不需要。新建 crate 增加 workspace 维护成本。如果未来 HTTP 也需要，再提取到独立 crate。

**替代方案**：放到 `cdt-query` 里。但 `cdt-query` 当前不依赖 `serde_json::Value`（view 层的 `ToolExecView` 需要），会改变 crate 的依赖边界。

### D2: `--content omit|full` 仅在显式指定时激活 view 输出

- 不指定 `--content`：保持当前 raw `SessionDetail` JSON 输出（完全兼容）
- `--content omit`：输出 `SessionDetailView` 包装，chunk 内容省略
- `--content full`：输出 `SessionDetailView` 包装，chunk 内容完整

**为什么不默认改为 view 输出**：虽然没有测试覆盖 JSON 结构，但可能有外部脚本依赖当前 raw JSON shape。显式 opt-in 零破坏性。

**`--content` 与 `--full` 的语义区分**：
- `--full`（改名 `--all`，原名作 alias）：控制 chunk 数量——"返回全部 chunk，禁用默认 tail=20"
- `--content`：控制内容模式——"JSON/JSONL 中是否包含正文/工具输入输出"
- 两者正交，可组合：`--all --content omit` = 所有 chunk 的结构概览

### D3: grep 顺序统一为 MCP 语义

改为 `kind_filter → grep/context → range/tail`（当前 CLI 是 `kind_filter + range/tail → grep`）。

**为什么统一**：当前 CLI 的 `--grep foo` 默认只在最后 20 个 chunk 内搜索（因为先 tail=20 再 grep），agent 会漏掉早期命中。MCP 语义是在全集上 grep 再 window，更符合用户预期。

**破坏性分析**：改变了 `--grep` 行为——之前是"最后 20 个 chunk 中搜索"，现在是"全局搜索后取最后 20 个命中"。这是语义改善，不是回归。

### D4: `--json <fields>` 字段选择（Phase 2）

类似 gh CLI 的 `--json fields` 模式：
- `--json sessionId,title,messageCount`：输出紧凑 JSON，只含指定字段
- `--json`（无参数）：列出可用字段名
- 隐含 `--format json`（不需要同时指定）

**实现方式**：序列化为 `serde_json::Value` 后做字段投影。对数组输出（如 `sessions list` 返回 `[{session1}, {session2}]`），投影作用于数组内每个元素的顶层 key，不是数组本身的顶层。对单对象输出（如 `sessions summary`），投影作用于对象顶层 key。输出紧凑 JSON（不 pretty-print）。

**未知字段处理**：指定了不存在的字段名时 SHALL 静默忽略（该字段不出现在输出中），不报错。这与 gh CLI 行为一致。

**与 `--content` 的交互**：`--json fields` 作用于序列化后投影，`--content` 作用于 view 构建时。两者可组合使用。顺序：domain → view(content mode) → serialize → field filter。例：`--content omit --json chunkIndex,toolExecutions` 先以 omit 模式构建 ChunkView（tool output 省略），再只保留 `chunkIndex` 和 `toolExecutions` 字段。

### D5: 重命名解决命名冲突

提取 view 层时需要解决 3 个与现有类型的命名冲突：
- `ChunkEnvelope` → `ChunkView`（通用命名）
- `ToolExecEnvelope` → `ToolExecView`
- `ResponseEnvelope` → `ResponseView`
- MCP 保留自己的 `SessionDetailMcpResponse`（原 `SessionDetailResponse`，避免与 `cdt_api::SessionDetailResponse` 冲突）
- MCP 保留自己的 `McpErrorEntry`（原 `ErrorEntry`，避免与 `cdt_query::ErrorEntry` 冲突）

### D6: `message_content_text` 合并策略

MCP 版本包含 `Thinking` block，`cdt-discover::search_text` 版本只收集 `Text` block。共享 view 层使用 MCP 版本（包含 Thinking），grep 场景的 text 提取仍用 `search_text` 版本。两者有不同的语义用途，不强制合并。

### D7: grep `--tail` 的作用对象

grep + context expansion 后产生"可见 chunk 集合"，`--tail N` 从这个可见集合中取最后 N 个。即 tail 的作用对象是"展开后的可见 chunk"，不是"直接命中"。这与 MCP 的分页行为一致（MCP 在 grep + context 展开后再分页）。

### D8: `truncate_display` 模块归属

`truncate_display()`（基于 `unicode-width` 计算 display width 的截断函数）放在 `view.rs` 中，虽然它主要服务 table 渲染。理由：MCP 的 `summarize_input()` 和 `truncate_str()` 也在 view 层，统一放置便于维护。table 特有的列宽分配逻辑不放 view，留在 `main.rs`。

## Risks / Trade-offs

- **[R1] `--content` 增加 CLI 概念负担** → 只在 JSON/JSONL 模式下生效；table 模式天然是 overview 不受影响；help 文本明确说明
- **[R2] grep 顺序改变是行为变更** → 语义改善（全局搜索 vs 窗口内搜索），PR 描述中标明
- **[R3] view 输出结构与 raw JSON 不同** → `--content` 是 opt-in，不指定时完全兼容；为 view 输出加 contract test
- **[R4] `--all` 改名可能影响现有脚本** → `--full` 作为 alias 保留，不删除
- **[R5] ContentMode 只影响序列化不影响内存** → 当前 `LocalDataApi` 总是加载全量数据。这是已知架构局限，不在本次改动范围。未来可在 DataApi 层优化
- **[R6] exit(0) 改变空结果语义** → agent 场景受益（不再误判为失败）；人类场景通过 stderr 提示 + 空 JSON 输出仍可感知

## Open Questions

- Phase 3 的 `terminal-size` 弹性列分配细节（各 table 的固定列 vs 弹性列比例）留到实现时决定

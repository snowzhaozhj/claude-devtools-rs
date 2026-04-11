# Baseline Cross-Check Findings

本文件汇总 baseline spec 与当前 TS 实现对齐过程中发现的偏差、疑似 bug、spec 未覆盖的真实行为，以及无法写入 spec 的 UI 隐式契约。供 Rust 重写时决策"复刻 vs 修正 vs 补 spec"。

图例：
- **spec-gap**：spec 描述不准确或缺失，应在 baseline 归档前或 Rust port 前更新 spec
- **impl-bug?**：疑似实现 bug，Rust port 时应修正而非复刻
- **coverage-gap**：spec 未覆盖但实现里有真实行为，需要补 scenario 或拆新 capability
- **implicit**：无法写进 baseline 的隐式契约（UI 交互、状态动画、键盘绑定等）

---

## session-parsing

### [impl-bug?] requestId 去重函数存在但未被调用 ✅ 已在 `port-session-parsing` 修正
- Spec: `Deduplicate streaming entries by requestId` requirement
- 代码：`src/main/utils/jsonl.ts` 定义了 `deduplicateByRequestId`，但 `src/main/services/parsing/SessionParser.ts:77` 附近的 `processMessages()` 未调用它
- 现状：流式 rewrite 场景下可能计入多条同 `requestId` 的 assistant 消息
- Rust port 决策：实现去重（按 spec），不复刻这个 miss
- **Rust 实现**：`crates/cdt-parse/src/dedupe.rs::dedupe_by_request_id` 由 `parse_file` 在收集完所有 `ParsedMessage` 后自动调用；`crates/cdt-parse/tests/dedupe.rs::parse_file_invokes_dedup_automatically` 是 wire-in 回归测试。

### [coverage-gap] 缺 JSONL 解析恶意输入的测试 ✅ 已在 `port-session-parsing` 补齐
- `test/main/services/parsing/` 没有对单行 malformed JSON 的用例
- Rust port 时应配套加 scenario-level test
- **Rust 实现**：`crates/cdt-parse/tests/parse_file.rs::{malformed_line_in_middle_is_skipped, two_adjacent_malformed_lines_both_skipped, empty_file_returns_empty_vec}` 覆盖全部三种异常路径；malformed 行通过 `tracing::warn!` 报告并跳过。

---

## chunk-building

### [impl-bug?] Task tool 过滤未在 AIChunk 构建阶段生效
- Spec: `Filter Task tool uses when subagent data is available`
- 代码：`ToolExecutionBuilder` 构建所有 tool execution，随后的 `ChunkFactory.buildAIChunkFromBuffer` 未在 subagent 已 resolve 的情况下移除对应 Task tool_use
- 可能结果：UI 里同一个 Task 既作为工具项展示，也作为 subagent 展示
- Rust port 决策：按 spec 过滤

### [coverage-gap] 多 tool 链接 / orphan tool_result / Task 过滤没有测试
- `test/main/services/analysis/ChunkBuilder.test.ts` 只覆盖基础 chunk 创建和 sidechain 过滤
- 补 vitest + Rust 实现 scenario

### [implicit] SemanticStepGrouper 的分组粒度未进 spec
- `SemanticStepExtractor` 提取 thinking/text/tool/subagent 步骤；`SemanticStepGrouper` 把相邻同类步骤合并展示
- baseline 只冻结"按时间顺序提取"，没有冻结合并策略（实现细节可能演进）
- Rust port 决策：自行设计分组策略，不约束

---

## tool-execution-linking

### [spec-gap] 重复 tool_use_id 的处理没被实现
- Spec 写了"WHEN 两个 tool_result 共享同一 id THEN 记录第一个并 log warning"
- 代码：`ToolExecutionBuilder` 没有 duplicate-id 检测与告警分支
- 决策：要么删掉 spec 的该 scenario（未实现），要么 Rust port 补实现。倾向 **保留 spec + Rust 补实现**（正确行为）

### [spec-gap] SendMessage summary 格式细节与实现不一致
- Spec: "SendMessage 摘要应含 recipient 和 truncated message preview"
- 代码 `src/renderer/utils/toolRendering/toolSummaryHelpers.ts:237` 使用 `type` 与 `to` 字段，不一定包含正文 preview
- 决策：Rust port 时按 spec 写；baseline spec 可保留不动

### [coverage-gap] Task→subagent 的三阶段匹配（result-based / description-based / positional）没写进 spec
- `SubagentResolver.ts:207-309` 实现了三级 fallback 匹配
- spec 只说"match task descriptions and spawn timestamps"过于笼统
- 建议：归档前给 `tool-execution-linking` 补一条 `Match Task calls to subagents by three fallbacks` 的 requirement

---

## team-coordination-metadata

### [spec-gap] teammate vs subagent 分开计数不在实现里
- Spec: "count distinct teammates separately from regular subagents"
- 代码：`SubagentResolver` 把 team 信息塞进 `Process.team`，但没有独立的 teammate 计数 API
- 决策：倾向 **修改 spec**，把该 scenario 改写为"能从 Process.team 区分 teammate 与普通 subagent，调用方自行计数"

### [coverage-gap] 缺 teammate detection / team enrichment 测试
- 现有测试没有覆盖 `isParsedTeammateMessage` 分支与 `Process.team` 富化链路
- Rust port 时应补

---

## project-discovery

### [spec-gap] 路径解码"最接近的存在路径"歧义消解没实现 ✅ 已在 `port-project-discovery` 修正
- Spec: `Path containing legitimate hyphens` → "resolving to the closest existing filesystem path when ambiguous"
- 代码：`src/main/utils/pathDecoder.ts:40-64` 是 best-effort 简单替换，注释明确说不能歧义消解；歧义靠 `ProjectPathResolver.ts:76-86` 通过读 JSONL 里的 `cwd` 补救
- 决策：**改 spec**，把机制写清楚：解码是 best-effort；真实路径由 session 文件中的 cwd 字段最终确定
- **Rust 实现**：`crates/cdt-discover/src/path_decoder.rs::decode_path` 保持 best-effort；`crates/cdt-discover/src/project_path_resolver.rs::ProjectPathResolver::resolve` 的解析顺序为 composite registry → cache → 绝对路径 hint → `read_lines_head` 抽 session `cwd` 字段 → `decode_path` fallback。集成测试 `cwd_field_overrides_decode` / `decode_path_fallback_used_when_no_cwd_in_sessions` 覆盖两条主路径。同时 port 在 `FileSystemProvider` 上新增 `read_lines_head`，修正 TS 侧 SSH 模式必须拉完整 JSONL 的隐性性能 bug。

---

## configuration-management

### [impl-bug?] 损坏 config 不会自动备份
- Spec: "back up the corrupted file, load defaults, log the error, and continue"
- 代码：`ConfigManager.ts:379-396 loadConfig()` 只 log + 加载默认，没有备份
- 决策：Rust port 时按 spec 实现备份行为

---

## context-tracking

### [spec-gap] Compaction 边界检测机制描述与实现不一致（行为一致）
- Spec 说"检测 compact summary messages"
- 代码：`contextTracker.ts:998` 通过 display item `type === 'compact'` 检测
- 两者行为等价（CompactChunk 总是对应 compact summary message），但机制描述需要对齐
- 决策：**微调 spec 措辞**为"context phase boundaries derived from compact items / compact summary messages"

### [spec-gap] notification-triggers spec 里的 `is_error` 检测路径可能偏离实现
- Spec: "detect by inspecting tool_result for is_error=true"
- 代码：`ErrorDetector.ts` 主要靠内容匹配 + 规则，没明确的 `is_error` 分支
- 决策：需要二次确认实现细节；若确实未检 `is_error`，倾向 **修实现**（spec 的行为更正确）

### [coverage-gap] computeContextStats / processSessionContextWithPhases 无单元测试
- `test/renderer/utils/` 下只有 `claudeMdTracker.test.ts`
- Rust port 时应补这两个核心函数的测试

---

## ipc-data-api

Spec 覆盖了 9 大操作集合，但 preload 真实暴露的 API 超出 spec 列表。**spec 未覆盖的真实 API**：

- `readAgentConfigs`（`src/preload/index.ts:180`）
- `getSessionsByIds`（`:157`）
- `getSessionGroups`（`:155`）
- `getRepositoryGroups` / `getWorktreeSessions`（`:161-163`）
- `readClaudeMdFiles` / `readDirectoryClaudeMd` / `readMentionedFile`（`:172-177`）
- `session.scrollToLine`（`:327`，UI 定位 deep link）

决策：**归档前给 ipc-data-api 补一条 requirement**，列出这些 API 的用途，或者显式把 CLAUDE.md 相关操作从 `configuration-management` 中移过来。`session.scrollToLine` 是 UI 定位，属于 UI 层隐式契约 → 放 implicit 区。

---

## http-data-api

### [spec-gap] 路由前缀与错误码全部与实现偏差
- Spec 示例用 `GET /projects`、`POST /search/sessions`；实现用 `/api/projects`、`/api/projects/:projectId/sessions-paginated` 等 `/api/*` 前缀
- Spec 约定 400/404/409/500；实现大量返回空数组/空对象/null，没有显式 HTTP 状态码区分
- 决策：
  1. **改 spec**：把前缀写成 `/api`，把路由形态贴近实现
  2. **Rust port 时修正错误处理**：按 spec 的 status code 约定实现

### [coverage-gap] 实现里存在但 spec 没列的路由
- `src/main/http/utility.ts`、`validation.ts` 等 12 个路由文件全部覆盖到，spec 只点名了一半
- 建议：归档前为 http-data-api 补一个"完整路由清单"附录，或拆出 `http-routes` 能力

---

## file-watching

✅ 完全匹配：100ms 去抖常量 `FileWatcher.ts:35 DEBOUNCE_MS = 100`，事件 payload 字段对齐，多订阅者分发 OK。无 followup。

---

## session-search

✅ 行为全对：scope、case-insensitive、noise 排除、cache by mtime。

### [coverage-gap] SSH stage-limit 快速搜索未进 spec
- `SessionSearcher.ts:29-31 SSH_FAST_SEARCH_STAGE_LIMITS` 在 SSH 模式下限制扫描阶段
- 决策：Rust port 时保留，spec 归档前加一条"SSH 模式下支持分阶段限制以避免长延迟"

---

## ssh-remote-context

✅ 完全匹配：`LocalFileSystemProvider` / `SshFileSystemProvider` 都实现同一 `FileSystemProvider` 接口；`ServiceContextRegistry.switch()` 支持切换；状态枚举齐全。无 followup。

---

## notification-triggers

见 context-tracking 区块下 `is_error` 那条；其它条目与实现匹配。

---

## Implicit contracts（baseline 外，UI 层）

下列行为无法冻结进 baseline specs，Rust 重写选 UI 技术栈时需要单独决策是否复刻：

- **滚动编排**（`useTabNavigationController`, auto-scroll bottom, scroll restore）
- **搜索高亮跨会话定位**（`SessionSearcher` + 滚动联动 + 高亮持久化）
- **Tab 导航与关闭历史**（`tabSlice` + `tabUISlice`，每 tab 独立 UI 状态隔离）
- **键盘快捷键**（`keyboardUtils`，Tab 切换、搜索焦点、复制）
- **Markdown 渲染细节**（`react-markdown` + `remark-gfm` + `mermaid` + 代码块 syntax highlight）
- **主题切换与 CSS 变量级联**（`useTheme`，dark/light）
- **Dashboard 水瀑图渲染策略**（`waterfall` 数据 → 渲染形态）
- **虚拟滚动 / 大会话渲染性能**（decision on list virtualization 策略）
- **Notification 桌面提醒 / 系统托盘** 行为

这些条目在 Rust port 里属于 **UI 技术栈决策域**，可以按新栈习惯重做，不强制 1:1。

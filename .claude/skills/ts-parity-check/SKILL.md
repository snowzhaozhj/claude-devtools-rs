---
name: ts-parity-check
description: 对比 TS 源（`../claude-devtools`）与 Rust 端口指定 capability 的文件映射，并查 `openspec/TS_BASELINE_DEVIATIONS.md` 看是否有该 capability 相关的 TS 偏差预警 + 当前 GitHub Issues backlog 里的相关项。**用户说 `/ts-parity-check <capability>` 或"对比一下 chunk-building 的 TS 与 Rust / 这个 cap 还有 TS 偏差吗"时都用这个 skill**——不要自己手 grep 比一遍。
---

# ts-parity-check

port 阶段（13 个 capability）已全部归档。这个 skill 现在的价值是：

- **回溯审查**：某 capability 在 port 时声称"已修"的 followup 条目实际有没有落地到 Rust 代码里
- **新 followup 评估**：发现一个新的 TS impl-bug 时，对照 Rust 实现确认是否已自动避开
- **重新 port 决策**：极少见——某 capability 想重写时先看 TS 与 Rust 当前的差距

如果 capability 还没 port，那直接走 `/opsx:propose port-<cap>` 流程，不需要这个 skill。

## 输入

一个 capability 名（kebab-case），例如 `chunk-building`、`tool-execution-linking`。

无参数时：用 `ls openspec/specs/` 列出所有 capability 让用户选。**不要**硬编码列表——specs 目录会随时间增删。

## 路径约定

- Rust 端口仓库根：`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/`
- TS 参考源：`/Users/zhaohejie/RustroverProjects/claude-devtools/`（已在 Claude Code 的 `additionalDirectories` 中允许读取）
- Spec：`openspec/specs/<capability>/spec.md`
- TS 偏差预警：`openspec/TS_BASELINE_DEVIATIONS.md`（grep capability 名 / 关键词；不再按章节切）
- 跨 cap backlog：`gh issue list --state open --search "<capability 关键词>"`

## 工作步骤

1. **定位 Rust owning crate**
   从 CLAUDE.md "Capability → crate map" 段（现在是一行 inline，不是表格）解析：

   - `cdt-parse`：session-parsing
   - `cdt-analyze`：chunk-building / tool-execution-linking / context-tracking / team-coordination-metadata
   - `cdt-discover`：project-discovery / session-search
   - `cdt-watch`：file-watching
   - `cdt-config`：configuration-management / notification-triggers
   - `cdt-ssh`：ssh-remote-context
   - `cdt-api`：ipc-data-api / http-data-api

2. **定位 TS 源文件**
   在 `/Users/zhaohejie/RustroverProjects/claude-devtools/src/main/` 下用 Grep/Glob 找匹配模块。常见映射：

   | capability | TS 目录/文件 |
   |---|---|
   | session-parsing | `services/parsing/SessionParser.ts`, `utils/jsonl.ts` |
   | chunk-building | `services/analysis/ChunkFactory.ts`, `ChunkBuilder.ts` |
   | tool-execution-linking | `services/analysis/ToolExecutionBuilder.ts`, `SubagentResolver.ts` |
   | context-tracking | `renderer/utils/contextTracker.ts` |
   | project-discovery | `services/discovery/`, `utils/pathDecoder.ts` |
   | session-search | `services/search/SessionSearcher.ts` |
   | file-watching | `services/watch/FileWatcher.ts` |
   | configuration-management | `services/config/ConfigManager.ts` |
   | notification-triggers | `services/notifications/ErrorDetector.ts` |
   | team-coordination-metadata | `services/team/` |
   | ssh-remote-context | `services/ssh/` |
   | ipc-data-api | `preload/index.ts`, `main/ipc/` |
   | http-data-api | `main/http/**` |

   若表中没有精确匹配，用 Grep 按 capability 关键词在 TS 源里搜索。

3. **读 spec、TS 偏差预警、open issues**
   - 完整读 `openspec/specs/<capability>/spec.md` 的 Requirements
   - 在 `openspec/TS_BASELINE_DEVIATIONS.md` 里 grep capability 名 / 关键词，列出该 cap 相关的 deviation / spec-gap / implicit 条目
   - 跑 `gh issue list --state open --search "<capability 关键词>"` 拿当前 backlog 里相关 issue（含 #230-#239 这批从原 followups.md 迁出的）

4. **对照 Rust 现状**
   - 在 owning crate 下用 Glob 列出所有 `.rs` 文件
   - 对每个 deviation / open issue：grep Rust 实现确认对应函数 / 模块状态；尤其留意 issue 描述里的"修法候选"在 Rust 里有没有 partial 实现

5. **输出报告**（≤ 500 字）

   ```
   # ts-parity-check: <capability>

   **Rust crate**：<crate>
   **Archive**：openspec/changes/archive/<日期>-port-<capability>/（若有）
   **Spec**：<scenario 数> 个 SHALL 行

   ## 文件映射
   | TS | Rust | 状态 |
   |---|---|---|
   | SessionParser.ts | crates/cdt-parse/src/parser.rs | ✓ |
   | jsonl.ts (dedupe) | crates/cdt-parse/src/dedupe.rs | ✓ |

   ## TS 偏差预警（TS_BASELINE_DEVIATIONS.md 命中）
   - [deviation] <摘要> — Rust：<状态>

   ## Open Issues（gh issue list）
   - #N <标题> — Rust：<相关位置>

   ## 建议
   - <若仍有相关 open issue>: 列出每个 issue 的下一步动作
   - <若 spec 与 Rust 实现不符 / TS deviation 加深>: ⚠️ 建议核对或开新 issue
   - <若全部一致>: 报告"全部落地"
   ```

## 硬性约束

- 只读：不改任何文件、不跑 cargo
- 引用 TS 或 Rust 文件时必须带**行号区间**——避免凭印象引用
- 如果用户没给 capability 名，用 `ls openspec/specs/` 列出选项让用户选（不要硬编码列表）
- 不要假装比较了没读过的文件——每个"✓"都必须对应一次实际的 Read/Grep
- 发现"open issue 描述与 Rust 实现已不匹配"——在报告里高亮，但**不要**自己 close issue / 改 Rust 代码；交给用户决定

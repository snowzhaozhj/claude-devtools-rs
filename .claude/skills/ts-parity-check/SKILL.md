---
name: ts-parity-check
description: 对比 TS 源（`../claude-devtools`）与 Rust 端口指定 capability 的文件映射，并列出 `openspec/followups.md` 里该 capability 下的 impl-bug / coverage-gap / deviation / implicit 条目 + 各自的 Rust 落地状态。**用户说 `/ts-parity-check <capability>` 或"对比一下 chunk-building 的 TS 与 Rust / 我们 port 这个 cap 时漏了什么 / 这个 cap 的 followups 都修了吗"时都用这个 skill**——不要自己手 grep 比一遍，容易漏 followups 章节里的"未修"条目。
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
- Followups：`openspec/followups.md`（按 `^## <capability>` 切章节）

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

3. **读 spec 和 followups**
   - 完整读 `openspec/specs/<capability>/spec.md` 的 Requirements
   - 在 `openspec/followups.md` 里抓该 capability 的 section（`## <capability>`），列出每条 `### [impl-bug?]` / `### [coverage-gap]` / `### [spec-gap]` / `### [deviation]` / `### [implicit]`
   - 区分"已修 ✅"（标题或正文含"✅ 已在 ... 修正"/"已修复" / "**Rust 实现**："）vs "pending"

4. **对照 Rust 现状**
   - 在 owning crate 下用 Glob 列出所有 `.rs` 文件
   - 对每个 followup 条目：grep Rust 实现确认对应函数 / 模块是否真的存在；尤其留意已标"✅"但实现可能漂移的条目

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

   ## Followups 清单
   - [impl-bug?] <摘要> — Rust：✅ 已修（grep 到 `<fn>` at <file:line>）
   - [coverage-gap] <摘要> — Rust：⚠️ 标了"✅"但 Rust 里找不到对应实现
   - [deviation] <摘要> — Rust：pending（建议补 spec scenario 或在 Rust 修正）

   ## 建议
   - <若仍有 pending followup>: 列出每条对应的下一步动作
   - <若发现"标 ✅ 但 Rust 找不到">: ⚠️ followup 引用漂移，建议核对
   - <若全部已修>: 报告"全部落地"
   ```

## 硬性约束

- 只读：不改任何文件、不跑 cargo
- 引用 TS 或 Rust 文件时必须带**行号区间**——避免凭印象引用
- 如果用户没给 capability 名，用 `ls openspec/specs/` 列出选项让用户选（不要硬编码列表）
- 不要假装比较了没读过的文件——每个"✓"都必须对应一次实际的 Read/Grep
- 发现"followup 标 ✅ 但 Rust 实现找不到"——在报告里高亮，但**不要**自己 Edit followups.md / Rust 代码；交给用户决定

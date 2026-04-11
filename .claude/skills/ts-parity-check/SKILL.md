---
name: ts-parity-check
description: 对比 TS 源（../claude-devtools）与 Rust 端口指定 capability 的文件映射，并列出 openspec/followups.md 里的相关 impl-bug 与 coverage gap。用于开始一个新 port 或审查已完成 port 时的快速差异检查。
---

# ts-parity-check

当用户说 `/ts-parity-check <capability>` 或 "帮我对比一下 chunk-building 的 TS 与 Rust 实现" 时触发。

## 输入

一个 capability 名（kebab-case），例如 `chunk-building`、`tool-execution-linking`。

## 路径约定

- Rust 端口仓库：`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/`
- TS 参考源：`/Users/zhaohejie/RustroverProjects/claude-devtools/`（已在 Claude Code 的 `additionalDirectories` 中允许读取）
- Spec：`openspec/specs/<capability>/spec.md`
- Followups：`openspec/followups.md`

## 工作步骤

1. **定位 Rust owning crate**
   读 `CLAUDE.md` 的"Capability → crate map"表，找到 capability 的 crate。

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
   - 完整读 `openspec/specs/<capability>/spec.md` 的 Requirements。
   - 在 `openspec/followups.md` 里抓该 capability 的 section（`## <capability>`），列出每条 `[impl-bug?]` / `[coverage-gap]` / `[spec-gap]`。

4. **对照 Rust 现状**
   - 在 owning crate 下用 Glob 列出所有 `.rs` 文件。
   - Grep 这些文件，判断每个 TS 关键类型/函数是否有对应物。

5. **输出报告**（≤ 500 字）
   结构：
   ```
   # ts-parity-check: <capability>

   **Rust crate**: <crate>
   **Port 状态**: not started / in progress / done（按文件存在推断）

   ## 文件映射
   | TS | Rust | 状态 |
   |---|---|---|
   | SessionParser.ts | crates/cdt-parse/src/parser.rs | ✓ |
   | jsonl.ts (dedupe) | crates/cdt-parse/src/dedupe.rs | ✓ |

   ## Followups 清单
   - [impl-bug?] <摘要> — Rust 当前状态：<已修 / 未处理 / N/A>
   - [coverage-gap] <摘要> — Rust 当前状态：...

   ## 建议
   - <若未开始>: 建议 `/opsx:propose port-<capability>`
   - <若进行中>: 还缺 X/Y scenario 测试
   - <若完成>: 提示未落地的 followup 条目
   ```

## 硬性约束

- 只读：不改任何文件、不跑 cargo。
- 引用 TS 或 Rust 文件时必须带行号区间。
- 如果用户没给 capability 名，用 `CLAUDE.md` 表列出 13 个选项让用户选。
- 不要假装比较了没读过的文件——每个"✓"都必须对应一次实际的 Read/Grep。

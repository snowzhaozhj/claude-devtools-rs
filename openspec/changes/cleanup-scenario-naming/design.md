## Context

issue #303 9-PR plan 阶段 3 第一个 PR。前序 PR 5（change cleanup-sidebar-navigation, PR #322）+ follow-up（change cleanup-spec-dangling-design-refs，issue #323）已 merge，sidebar-navigation 主 spec 干净。本 change 是跨 cap 的 Scenario 标题命名扫描清理。

工艺直接复用：

- change `cleanup-sidebar-navigation`（PR #322）—— 单 cap 14 Requirement 重写
- change `cleanup-config-and-context-menu`（PR #319）—— 双 cap 合并 1 PR
- change `ssh-remote-context-cleanup`（PR #312）—— 14 Requirement 重写

相比 PR #312 / #319 / #322 改动量更小（20 Scenario 跨 9 cap / 16 Requirement，每 Requirement 仅 1-3 Scenario 标题改名），但跨度更广（首次 cross-cap Scenario naming 扫描）。

## Goals / Non-Goals

**Goals:**

- 20 个明显"内部 symbol 视角"Scenario 标题改用户 / 系统可观察行为视角，符合 `SPEC_GUIDE.md::反例 1` + reviewer checklist 末两条
- 每个 Requirement body + 每条 SHALL / MUST / WHEN / THEN / AND 句 100% 不变（语义对等）
- Requirement 数量不变；各 cap Scenario 数量不变

**Non-Goals:**

- 不改代码 / 测试 / 配置
- 不改 IPC 字段名 / Tauri command 名 / SSE event 名 / HTTP 路径 / 错误 variant 名等协议契约
- 不改 `session-display` / `sidebar-navigation` / `ipc-data-api` 三个待拆 cap（在后续拆分 PR 7/8/9 内一起做）
- 不批量重写所有 Scenario 标题——只改"明显内部 jargon"（标题里有内部 fn 名 / mod 路径 / Rust 类型签名 / 内部 const / lib 名 / 内部 channel 名）；微妙边界（cap 内部协议术语、文档级风格用语）保留
- 不动 Purpose 段
- 不动 Requirement 标题（标题级 RENAMED 工艺成本相对收益不划算，留作后续 spec 重组 PR 一起处理）
- **不顺手清理被 MODIFIED 的 Requirement body 历史污染**——本 PR scope 严格限定为"仅替换 `#### Scenario:` 这一行"。`SPEC_GUIDE.md::改既有 spec 的判断顺序` 默认期望"遇到一个修一个"，但本 PR 一次性 MODIFIED 16 个 Requirement，若同步清 body 反模式（如 `fs-abstraction::本 change 零业务变化下性能基线不退化` body 内的 `Box<dyn AsyncRead>` / `crates/cdt-fs/benches/...` / `cargo test --release -p cdt-api ...` 等历史污染），单 PR 体量过大、reviewer 字符级对照成本陡升、出错风险变高。这些 body 清理留给后续**单 cap 重组 PR**或**专门的 body cleanup PR**逐个处理。本 PR 通过 spec-purity baseline 注册 `change/cleanup-scenario-naming/<cap>` 行登记 active change 内已知 body 反模式数（继承自主 spec），不增、不减、不刷新主 spec baseline；archive 后这些 entry 随 active change mv 走自然清理。

## Decisions

### D-1：行为契约 100% 不变

**问题**：Scenario 标题改名相比 Requirement body 重写表面更轻，但若不小心同步改了 SHALL 句或 WHEN/THEN 子句，仍可能破契约。

**决策**：每个 MODIFIED Requirement 严格按"复制原文 → 仅替换 `#### Scenario: <旧标题>` 这一行"原则操作。Requirement body / 每条 Scenario 内的 WHEN/THEN/AND 子句字符级等价。

### D-2：扫描范围与判断标准

**扫描范围**：所有 active capability spec.md（共 28 cap，扫描了所有 `#### Scenario:` 行），排除 `session-display` / `sidebar-navigation` / `ipc-data-api` 三个待拆 cap。共 ~810 个 Scenario 标题。

**判断标准**（保守版）：以下任一命中即列入改名候选：

- 标题含内部 fn 名（`build_chunks` / `parse_file` / `dedupe_by_request_id` / `check_messages_ongoing` / `entry point`）
- 标题含 mod 路径或 crate 名（`cdt-discover` / `cdt-core::Session`）
- 标题含 Rust 类型 / trait bound（`Copy + Eq` / `Trait is the sole seam` / `ProjectPathResolver`）
- 标题含库名（`tracing emit` / `Library consumer` / `API`）
- 标题含内部 channel / const / 算法术语（`always-keep 通道` / `半压缩` / `Synthetic`）
- 标题含 CSS class / DOM 实现术语（`zone-drag-flex`）

**不改名**：

- 协议契约语言（IPC payload type 名 / Tauri command 名 / SSE event 名 / 错误 variant 名）—— 这是外部 owner spec 守护的协议
- cap 自身核心概念（如 `application-telemetry` 内的 `hot path counter` / `low-frequency event push`、`fs-abstraction` 内的 `open_read` / `stat_many` / `BackendPolicy`、`session-parsing` 内的 `interrupt marker`）—— 这些是 cap 对外承诺的 API 与术语
- 微妙边界（如 `frontend-context-menu` 内 `factory 返回纯数据` / `trigger 元素 destroy 时菜单兜底卸载`）—— 改名收益相对模棱、留给后续 cap 内重构 PR

### D-3：改名决策表（19 case 跨 8 cap / 15 Requirement）

按 cap 分组，每行一项：

| # | cap | Requirement | 旧 Scenario 标题 | 新 Scenario 标题 | 触发标准 |
|---|---|---|---|---|---|
| 1 | app-chrome | chrome 四 zone 布局 | `zone-drag-flex 拖窗` | `拖动 chrome 非按钮区域移窗` | CSS 内部命名 |
| 2 | application-telemetry | panic critical event always-keep 通道 | `panic 触发 always-keep 通道写入` | `panic 触发关键事件入队` | 内部 channel 名 `panic_critical_event_channel` |
| 3 | application-telemetry | panic critical event always-keep 通道 | `panic 通道满时半压缩保留` | `panic 队列满时丢弃最老 50% 保留新事件` | 内部算法术语 "半压缩" |
| 4 | chunk-building | Link tool uses to tool results | `Tool executions populated by build_chunks` | `Tool executions populated during chunk build` | 内部 fn 名 `build_chunks` |
| 5 | chunk-building | Filter Task tool uses when subagent data is available | `Default build_chunks does not filter Tasks in this port` | `Default chunk build does not filter Tasks in this port` | 同上 |
| 6 | context-tracking | Expose a pure synchronous API driven by chunk output | `Library consumer calls the API from a sync context` | `Caller invokes context stats from a sync context` | 内部接口术语 `Library consumer` / `API` |
| 7 | fs-abstraction | `BackendPolicy` enum 雏形定义 | `BackendPolicy 是 Copy + Eq 类型` | `BackendPolicy 可按值复制并相等比较` | Rust trait bound `Copy + Eq` |
| 8 | fs-abstraction | Provider instrumentation 入口可观测 fs op 次数 | `tracing emit on Drop` | `wrapper 释放时输出诊断` | 库名 `tracing` + Rust Drop |
| 9 | fs-abstraction | 本 change 零业务变化下性能基线不退化 | `open_read dyn 路径 micro bench 不超 1.3x` | `open_read 动态分发路径开销不超单态化的 1.3x` | Rust 内部术语 `dyn 路径 micro bench` |
| 10 | project-discovery | Abstract filesystem access through a provider trait | `Trait is the sole seam for alternative backends` | `fs 抽象 trait 是替换 backend 的唯一接口` | Rust 内部术语 `Trait` / `seam` |
| 11 | project-discovery | Abstract filesystem access through a provider trait | `cdt-discover 继续兼容老 import` | `discover capability 暴露兼容 alias 给老调用方` | crate 名 `cdt-discover` + Rust `import` |
| 12 | project-discovery | Compare paths case-insensitively on Windows | `跨大小写命中同一 ProjectPathResolver 缓存` | `跨大小写命中同一项目路径解析缓存` | Rust 内部 type 名 `ProjectPathResolver` |
| 13 | project-discovery | Expose session cwd for downstream display | `cdt-core::Session 不含 cwd_relative_to_repo_root 字段` | `Session payload 不含 cwd_relative_to_repo_root 字段` | mod 路径 `cdt-core::Session` |
| 14 | session-parsing | Deduplicate streaming entries by requestId | `parse_file 保留同 requestId 的所有记录` | `解析文件时保留同 requestId 的所有记录` | 内部 fn 名 `parse_file` |
| 15 | session-parsing | Deduplicate streaming entries by requestId | `dedupe_by_request_id 仍作为 metrics 辅助函数可用` | `metrics 辅助路径仍可按 requestId 去重` | 内部 fn 名 `dedupe_by_request_id` |
| 16 | session-parsing | Expose both a per-line and a per-file parsing API | `Per-line entry point parses a valid assistant message` | `Per-line parse path handles a valid assistant message` | 内部接口术语 `entry point` |
| 17 | session-parsing | Expose both a per-line and a per-file parsing API | `Per-file entry point agrees with per-line entry point` | `Per-file parse path agrees with per-line parse path` | 同上 |
| 18 | session-parsing | Classify hard noise messages | `Synthetic assistant placeholder` | `Missing assistant generates placeholder` | 内部术语 `Synthetic` |
| 19 | tool-execution-linking | Enrich subagent processes with team metadata | `is_ongoing 判定走 check_messages_ongoing 算法` | `is_ongoing 判定按消息状态推算` | 内部 fn 名 `check_messages_ongoing` |
| 20 | tab-management | Pane 生命周期 | `Split 达到 MAX_PANES 上限` | `Split 达到最大 pane 数上限` | 内部 const 名 `MAX_PANES`（spec-guide-reviewer W1 补加） |

### D-4：保留的微妙边界（不改名理由）

扫描中遇到一些"看起来内部但实属 cap 自身术语"的标题，刻意保留：

- `application-telemetry::hot path counter 增量` / `hot path histogram 观察` / `低频路径 event push` —— `Counter` / `Histogram` / `Event` 是该 cap 对外承诺的三类信号类型，`hot path` / `低频路径` 是该 cap 对外的 API 分层契约
- `application-telemetry::既有 tracing::error 自动归 counter` —— `tracing::error` 这里指**用户代码已有的日志调用**而非 telemetry 内部实现选择，是 cap 与用户代码的边界契约
- `chunk-building::AIChunk with thinking + text + tool` 等 —— `AIChunk` / `UserChunk` / `CompactChunk` 是 IPC payload type（协议）
- `fs-abstraction::open_read 在 Local 上返回流式句柄` —— `open_read` 是该 cap 对外承诺的 trait method 名（cap 自身核心概念）
- `project-discovery::Local filesystem provider satisfies the scanner` —— `provider` / `scanner` 是该 cap 内部抽象但属 cap 主线概念
- `keyboard-shortcuts::dispatcher bubble phase 让组件 listener 先命中` —— DOM 概念用户视角可读
- `tool-execution-linking::装载层与主 session ongoing 判定一致` —— `装载层` 是 cap 内 subagent 加载抽象，已是用户视角抽象后的描述
- `frontend-context-menu::factory 返回纯数据` / `trigger 元素 destroy 时菜单兜底卸载` —— 改名收益模棱

这些微妙边界在后续 cap 内重构 PR（PR 7/8/9）或独立 cap 重组时再 case-by-case 重审，不在本 PR 一刀切。

### D-5：与 SPEC_GUIDE 反例 1 / 5 的关系

- **反例 1（日志调用作为 Scenario AND 子句）**：本 change 不直接命中——反例 1 针对的是 SHALL / AND 句体内含 `tracing::error!(target: ..., ...)` 这类绑死 log crate 的句子；本 change 只动 Scenario 标题，AND 句体不动
- **反例 5（主 spec 引用 design.md / PR # / archived change）**：本 change 不动 SHALL 句，无新增内引；保留主 spec 自含可读性
- 本 change 主要落在 reviewer checklist 末两条："Scenario 标题是否描述用户 / 系统可观察行为，没用 const / 回滚开关 / 实现术语命名？" + "清理 PR 命名扫描自检：Requirement title 是否与 body 同步抽象（避免 title 残留内部类型名 / 库名）？"

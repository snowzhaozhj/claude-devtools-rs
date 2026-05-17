## ADDED Requirements

### Requirement: Title prefers slash command with non-empty args over later non-command messages

`extract_session_metadata` 提取 `SessionSummary.title` 时 SHALL 把"带非空 `<command-args>` 内容的 slash 命令消息"视作真正的用户输入直接作为 title，而**不**降级到 `command_fallback`。空 args / 无 `<command-args>` tag 的纯辅助 slash（如 `/clear` / `/help` / `/cost`）SHALL 继续走 `command_fallback`，仅在所有非命令 user 消息都不可用时使用。

实现路径（`crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_with_ongoing`）：

1. `is_command_content(&text)` 命中时 SHALL 调用 `extract_command_display(&text)` 解析出 `/name` 与 `args` 两部分；
2. **若 `args` trim 后非空** → 拼成 `/name <args>` 字符串，按既有 `truncate_str(..., TITLE_MAX_CHARS)` 截断后**直接**赋值给 `title`（与普通文本消息走同一截断路径）；
3. **若 `args` 为空或缺失** → 按原逻辑写入 `command_fallback`（仅在循环结束 `title.is_none()` 时被使用）。

`command_fallback` 字段语义 SHALL 保持："仅在所有候选消息都被跳过时的兜底"。

#### Scenario: Slash command with non-empty args becomes the title

- **WHEN** session 第一条非 meta user 消息 content 为 `<command-name>/impeccable</command-name><command-args>根据项目的已有代码生成一下设计规范</command-args>`
- **AND** 第二条 user 消息 content 为 `提一下PR吧，我审查一下`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("/impeccable 根据项目的已有代码生成一下设计规范")`
- **AND** SHALL NOT 为 `Some("提一下PR吧，我审查一下")`

#### Scenario: Bare slash command with empty args falls back to next user message

- **WHEN** session 第一条非 meta user 消息 content 为 `<command-name>/clear</command-name><command-args></command-args>`（空 args）
- **AND** 第二条 user 消息 content 为 `今天的工作总结一下`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("今天的工作总结一下")`
- **AND** `command_fallback` 候选 `/clear` SHALL NOT 被使用

#### Scenario: Bare slash command without command-args tag falls back

- **WHEN** session 第一条非 meta user 消息 content 仅为 `<command-name>/help</command-name>`（无 `<command-args>` tag）
- **AND** 没有任何其他非命令 user 消息
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("/help")`（fallback 路径）

#### Scenario: Slash with args truncates at TITLE_MAX_CHARS

- **WHEN** session 第一条 user 消息为 `<command-name>/foo</command-name><command-args>` + 600 字符的中文 prompt + `</command-args>`
- **THEN** `extract_session_metadata.title.unwrap()` 的 `chars().count()` SHALL ≤ `TITLE_MAX_CHARS`（500）

### Requirement: Sanitize title against interruption and task-output instructions

`extract_session_metadata` 提取 `SessionSummary.title` 时 SHALL 跳过 / 清洗以下两类系统注入文本，避免它们污染 sidebar 标题：

1. **`[Request interrupted by user` 起首消息**：trim 后的文本以该字面量起首时，整条 user 消息 SHALL 不参与 title 提取（既不进 `title` 也不进 `command_fallback`），扫描循环 SHALL 继续找下一条候选。
2. **`Read the output file to retrieve the result: <path>` 指令残留**：`sanitize_for_title` 在已有 8 个 system tag + `teammate-message` 标签剥除后，SHALL 追加一次正则替换移除符合 `/ ?Read the output file to retrieve the result: \S+/g` 模式的所有匹配段（path 为非空白字符序列）。

实现位置：

- interrupted 过滤 SHALL 加在 `extract_session_metadata_with_ongoing` 内 `is_command_output(&text)` 判断同位置（前后均可），早于 `is_command_content` 与 `extract_teammate_summary_title`，以避免该字面量进入 `command_fallback` 与 teammate 路径。
- "Read the output file" 移除 SHALL 在 `sanitize_for_title` 函数尾部、`trim().to_string()` 之前应用；正则 SHALL 用 `once_cell::sync::Lazy<Regex>` 进程级编译复用。

#### Scenario: Interrupted message is skipped during title extraction

- **WHEN** session 第一条 user 消息 content 为 `[Request interrupted by user during tooling cycle]`
- **AND** 第二条 user 消息 content 为 `继续处理之前的任务`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("继续处理之前的任务")`
- **AND** SHALL NOT 含 `[Request interrupted`

#### Scenario: Read the output file instruction is stripped from title

- **WHEN** session 第一条 user 消息 content 为 `<task-notification>已完成</task-notification> Read the output file to retrieve the result: /tmp/result.txt`
- **THEN** `extract_session_metadata.title` SHALL 不含 `Read the output file to retrieve the result:` 子串
- **AND** SHALL 不含 `/tmp/result.txt`

#### Scenario: Multiple Read-output instructions all stripped

- **WHEN** session 第一条 user 消息含两段 `Read the output file to retrieve the result: /a` 与 `Read the output file to retrieve the result: /b`
- **THEN** `extract_session_metadata.title` 中两段 SHALL 全部被移除

## MODIFIED Requirements

### Requirement: Strip teammate-message tags from session title

`extract_session_metadata` 提取的 `SessionSummary.title` MUST 在做长度截断之前剥除任何 `<teammate-message ...>...</teammate-message>` 包裹片段，避免 sidebar 标题吐出原始 XML。

实现 SHALL 在 `cdt-api::session_metadata::sanitize_for_title` 同函数内完成两步：

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，先 regex 抽 `summary="..."` 属性内容；非空时 SHALL 直接返回 summary 内容作为标题候选（截断长度由常量 `TITLE_MAX_CHARS` 控制，见本 spec 同名 Requirement）。
2. **Fallback（剥标签）**：若 fast-path 未命中（无 summary 属性 / 文本含混合内容），SHALL 在既有标签剥除循环中追加 `teammate-message` 标签——把整段 `<teammate-message ...>body</teammate-message>` 从文本中删除（含 attributes 与 inner body）。剥除后若文本为空，SHALL 回退到 `command_fallback` 或 `None`，按既有路径处理。

`sanitize_for_title` MUST 不再在标题里输出任何 `<teammate-message` / `</teammate-message>` 字面量。

#### Scenario: Title takes summary attribute when message is wrapped solely by teammate-message
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice" summary="Set up project">body</teammate-message>`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("Set up project")`

#### Scenario: Title falls back when teammate-message has no summary
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice">body</teammate-message>`（无 summary 属性）
- **THEN** `extract_session_metadata.title` SHALL NOT 含 `<teammate-message`，且 SHALL 退回 `None` 或 `command_fallback`

#### Scenario: Mixed content strips teammate-message tag
- **WHEN** 第一条 user 消息 content 为 `Hello team. <teammate-message teammate_id="alice">body</teammate-message> please continue.`
- **THEN** title SHALL 不含 `<teammate-message`，剥除后 SHALL 仅保留 `Hello team.  please continue.`（trim 后），整体走既有截断路径

## ADDED Requirements

### Requirement: Title length is bounded by TITLE_MAX_CHARS constant

`extract_session_metadata` 提取的 `SessionSummary.title` 最终字符数 SHALL ≤ `TITLE_MAX_CHARS = 500`（Unicode `char` 计数，不是 byte 数）。所有截断路径（teammate summary fast-path / slash-with-args 直接路径 / 普通 sanitize 路径）SHALL 调用同一 `truncate_str(_, TITLE_MAX_CHARS)` helper，禁止散落不同 magic number。

常量 `TITLE_MAX_CHARS` SHALL 定义在 `crates/cdt-api/src/ipc/session_metadata.rs` 顶部并 `pub` 暴露给同 crate 测试。

#### Scenario: Plain-text title longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息 content 为 700 个中文字符的纯文本
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

#### Scenario: Slash with args longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息为 `<command-name>/foo</command-name><command-args>` + 700 字符 + `</command-args>`
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

### Requirement: Title algorithm changes do not invalidate MetadataCache

`extract_session_metadata` 的 title 提取算法（含 slash 处理 / interrupted 过滤 / sanitize 规则 / 截断长度）发生变化时 SHALL NOT 主动 invalidate `MetadataCache`。命中旧 `FileSignature`（mtime / size / identity 全部不变）的条目 SHALL 继续返回缓存里的旧 title 字符串，直到文件签名发生变化（用户写入新行）或被 LRU 淘汰后才按新算法重扫并写回。

理由：title 算法变更属于"对老 session 文件展示形态的语义优化"，老缓存按旧算法计算的 title 在用户视角上"不够好但不离谱"；强制 invalidate 会触发下次启动时数百 session 文件的扫描风暴（违反 perf 预算）。新会话 / 文件改动后的会话天然走新算法。

实现含义：

- `MetadataCache` 数据结构 SHALL NOT 因 title 算法版本变化而新加 `algorithm_version` 字段或类似 cache-busting 机制
- `LocalDataApi` SHALL NOT 在启动 / 配置变更 / app 升级路径触发 `cache.clear()` 等批量 invalidate
- 单条 cache miss 的判定 SHALL 仅依据 `FileSignature != stored.signature`（既有行为）

#### Scenario: Stored cache entry with old title is reused on hit

- **GIVEN** `MetadataCache` 已存在某 path 的 entry，其 `title = Some("旧规则算出的 title")`，`signature` 与磁盘文件当前 `FileSignature` 一致
- **WHEN** `extract_session_metadata_cached` 被以同一 path 再次调用
- **THEN** 返回的 `SessionMetadata.title` SHALL 等于 `Some("旧规则算出的 title")`
- **AND** 实现 SHALL NOT 重新读取或重新解析该 session JSONL 文件

#### Scenario: New title algorithm applies only to fresh scans

- **GIVEN** 同一 session JSONL 文件，缓存中存的旧 title 是 `"提一下PR吧，我审查一下"`（按旧算法）
- **WHEN** 该 session 文件被追加新内容导致 `FileSignature` 变化（mtime / size 改变）
- **THEN** 下一次 `extract_session_metadata_cached` SHALL 触发重扫
- **AND** 返回的 title SHALL 按新算法重新计算（截图 case 应得 `/impeccable 根据项目的已有代码生成一下设计规范`）

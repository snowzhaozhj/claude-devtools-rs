## ADDED Requirements

### Requirement: CLI 导出路径

CLI `cdt export <session-id>` SHALL 支持将会话导出为 Markdown 和 JSON 两种格式。CLI 导出是 in-process 调用，不经 IPC/HTTP，直接从本地文件系统读取完整 SessionDetail（含 tool output + response content）。

#### Scenario: CLI 导出为 Markdown

- **WHEN** 用户运行 `cdt export <session-id> --export-format md`
- **THEN** 输出 SHALL 包含 `# 标题` + 元数据表（Session ID / 工作目录 / 消息数 / 模型 / cost / 时长）+ 按 turn 分段的对话内容
- **AND** 每个 turn SHALL 渲染为 `## Turn N — {Role}`（User / Assistant / System / Context Compacted）
- **AND** Assistant turn 中的工具调用 SHALL 渲染为 `### Tool: {toolName}` 三级标题
- **AND** 默认输出到 stdout

#### Scenario: CLI 导出为 JSON

- **WHEN** 用户运行 `cdt export <session-id> --export-format json`
- **THEN** 输出 SHALL 为经投影处理的 SessionDetail JSON（pretty-print, 2-space indent）
- **AND** `--no-thinking` 时 thinking steps SHALL NOT 出现在 JSON 中
- **AND** `--detail name-only` 时 tool execution 的 input/output SHALL 为空

#### Scenario: CLI 导出写文件

- **WHEN** 用户运行 `cdt export <session-id> -o path/to/file.md`
- **THEN** 导出内容 SHALL 写入指定文件路径
- **AND** 成功时 SHALL 在 stderr 输出确认信息

#### Scenario: CLI 导出默认格式

- **WHEN** 用户运行 `cdt export <session-id>` 不指定 `--format`
- **THEN** 格式 SHALL 默认为 Markdown

#### Scenario: CLI 导出支持 latest 别名

- **WHEN** 用户运行 `cdt export latest`
- **THEN** SHALL 解析为最近一次 session 并导出

### Requirement: CLI 导出工具输出详略控制

CLI 导出 SHALL 支持 `--detail <full|summary|name-only>` 控制工具调用的渲染详略程度。

#### Scenario: --detail full 完整输出

- **WHEN** 用户以 `--detail full` 导出
- **THEN** 每个工具调用 SHALL 渲染完整 input 和 output

#### Scenario: --detail summary 截断输出

- **WHEN** 用户以 `--detail summary` 导出
- **THEN** 工具 output 超过 2000 字符时 SHALL 截断并附加 `... (truncated)`
- **AND** 截断 SHALL 按 Unicode scalar boundary 执行

#### Scenario: --detail name-only 仅名称

- **WHEN** 用户以 `--detail name-only` 导出
- **THEN** 仅 SHALL 渲染工具名称，不包含 input 和 output

#### Scenario: --detail 默认值

- **WHEN** 用户不指定 `--detail`
- **THEN** SHALL 默认为 `full`

### Requirement: CLI 导出可选内容控制

CLI 导出 SHALL 支持 `--no-thinking` 排除 thinking blocks 和 `--no-subagents` 排除子代理卡片。

#### Scenario: --no-thinking 排除 thinking

- **WHEN** 用户以 `--no-thinking` 导出
- **THEN** thinking blocks SHALL NOT 出现在导出内容中

#### Scenario: 默认包含 thinking

- **WHEN** 用户不指定 `--no-thinking`
- **THEN** thinking blocks SHALL 渲染为 `> [thinking] ...`（Markdown）或保留在 JSON 中

#### Scenario: --no-subagents 排除子代理

- **WHEN** 用户以 `--no-subagents` 导出
- **THEN** 子代理卡片 SHALL NOT 出现在导出内容中

### Requirement: CLI 导出与过滤参数组合

CLI 导出 SHALL 与现有 chunk 过滤参数正交组合：`--range` / `--tail` / `--grep` / `--filter`。过滤管道顺序 SHALL 与 `cdt session --chunks` 一致：kind_filter → grep → range/tail。

#### Scenario: --range 限定导出范围

- **WHEN** 用户运行 `cdt export <id> --range 10:20`
- **THEN** 导出 SHALL 仅包含 chunk index [10, 20) 范围内的 turn

#### Scenario: --grep 过滤导出内容

- **WHEN** 用户运行 `cdt export <id> --grep "authentication"`
- **THEN** 导出 SHALL 仅包含匹配 "authentication" 的 chunk 及其 context

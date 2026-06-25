# session-export Specification

## Purpose
TBD - created by archiving change session-export. Update Purpose after archive.
## Requirements
### Requirement: 导出格式与入口

用户 SHALL 能从 SessionDetail 视图的 SessionMetaMenu ("..." 菜单) 将当前会话导出为 Markdown / JSON / HTML 三种格式的文件。

#### Scenario: 点击导出为 Markdown

- **WHEN** 用户在 SessionMetaMenu 中点击「导出为 Markdown」
- **THEN** 系统拉取当前 session 的完整 SessionDetail（不使用 fingerprint 缓存）
- **AND** 生成包含元数据表 + 分 turn 结构化 markdown 的 `.md` 文件内容
- **AND** 弹出原生文件保存对话框（默认文件名 `session-{sessionId}.md`）
- **AND** 用户选择路径后写入文件
- **AND** 显示成功 toast 反馈

#### Scenario: 点击导出为 JSON

- **WHEN** 用户在 SessionMetaMenu 中点击「导出为 JSON」
- **THEN** 系统拉取完整 SessionDetail
- **AND** 将 SessionDetail 对象 JSON 序列化（pretty-print, 2-space indent）
- **AND** 弹出原生文件保存对话框（默认文件名 `session-{sessionId}.json`）
- **AND** 用户选择路径后写入文件

#### Scenario: 点击导出为 HTML

- **WHEN** 用户在 SessionMetaMenu 中点击「导出为 HTML」
- **THEN** 系统拉取完整 SessionDetail
- **AND** 生成自包含 HTML 文件（内嵌 CSS + JS，无外部依赖）
- **AND** HTML 中的 markdown 内容经 marked 渲染、代码块经 highlight.js 高亮
- **AND** 弹出原生文件保存对话框（默认文件名 `session-{sessionId}.html`）
- **AND** 用户选择路径后写入文件

#### Scenario: 用户取消保存对话框

- **WHEN** 用户在文件保存对话框中点击取消
- **THEN** 不写入任何文件
- **AND** 不显示错误 toast

#### Scenario: 写入失败

- **WHEN** 文件写入因权限或磁盘空间等原因失败
- **THEN** 显示错误 toast 提示「导出失败」

### Requirement: HTML 导出交互功能

导出的 HTML 文件 SHALL 提供轻量交互能力（内嵌 JS，无外部依赖），方便接收者浏览长会话。

#### Scenario: 工具详情折叠展开

- **WHEN** 接收者打开导出的 HTML 文件
- **THEN** 工具调用的 input/output 默认折叠，仅显示工具名和摘要
- **AND** 点击可展开查看完整内容

#### Scenario: 思考链折叠

- **WHEN** HTML 中包含 thinking blocks
- **THEN** thinking 内容默认折叠（显示「Thinking...」标签）
- **AND** 点击可展开查看完整思考过程

#### Scenario: 暗亮主题切换

- **WHEN** 接收者点击 HTML 右上角的主题切换按钮
- **THEN** 页面在亮色和暗色主题间切换
- **AND** 选择记忆到 localStorage（再次打开同一文件保持上次选择）

#### Scenario: 目录导航

- **WHEN** HTML 包含多个 turn
- **THEN** 页面左侧显示目录导航（列出每个 turn 的角色 + 序号）
- **AND** 点击目录项滚动到对应位置

### Requirement: Markdown 导出内容结构

Markdown 导出 SHALL 包含完整的会话结构信息，每个 turn 渲染为独立的二级标题段落。

#### Scenario: Markdown 元数据表

- **WHEN** 导出的 markdown 文件被打开
- **THEN** 顶部包含 session 元数据表（项目名、分支、时间范围、消息数、token 用量）

#### Scenario: Markdown turn 结构

- **WHEN** session 包含多个对话轮次
- **THEN** 每个 turn 渲染为一个二级标题（`## Turn N — {Role}`）
- **AND** User turn 包含用户消息原文
- **AND** Assistant turn 包含回复正文 + 工具调用（三级标题）

#### Scenario: thinking 可选包含

- **WHEN** 导出选项中 includeThinking 为 true
- **THEN** thinking blocks 渲染为 blockquote（`> [thinking] ...`）
- **WHEN** includeThinking 为 false
- **THEN** thinking blocks 不出现在导出内容中

#### Scenario: 工具输出截断模式

- **WHEN** toolOutputMode 为 "truncated"
- **THEN** 工具输出超过 toolOutputMaxLength 字符时截断并附加 `... (truncated)`
- **WHEN** toolOutputMode 为 "full"
- **THEN** 工具输出完整包含不截断
- **WHEN** toolOutputMode 为 "name-only"
- **THEN** 仅显示工具名称，不包含 input/output

### Requirement: HTTP 模式降级

非 Tauri 运行时（HTTP server 模式）SHALL 使用浏览器原生下载机制替代原生对话框。

#### Scenario: 浏览器模式下载

- **WHEN** 应用在 HTTP 模式下运行（非 Tauri desktop）
- **AND** 用户点击导出
- **THEN** 使用 Blob + `<a download>` 触发浏览器下载
- **AND** 文件名与 Tauri 模式一致（`session-{sessionId}.{ext}`）

### Requirement: 导出状态反馈

导出操作 SHALL 提供清晰的进行中/成功/失败状态反馈，防止用户重复操作。

#### Scenario: 导出进行中

- **WHEN** 导出操作开始（数据拉取 + 内容生成）
- **THEN** 菜单项文字变为「导出中...」
- **AND** 禁止重复点击

#### Scenario: 导出成功

- **WHEN** 文件写入成功
- **THEN** 显示「已导出」成功 toast（持续 1.5s 自动消失）

### Requirement: 子代理内容导出

当 session 包含子代理（subagent / workflow）时，导出 SHALL 按 `includeSubagents` 选项决定是否在父 turn 内就地渲染子代理卡片。子代理卡片 MUST 按 spawn 时间穿插在对话流中（复用 `buildDisplayItems` 时序），并对齐视图行为：发起该子代理的 Task / Agent 工具调用 SHALL 被去重（由子代理卡片代表），不重复渲染为普通工具。

> 注：导出**子代理内部对话消息**（嵌套 conversation 完整展开）尚未实现，与 teammate / slash / workflow 渲染缺失一并由范围外 issue 跟踪。本 Requirement 当前只约束子代理卡片摘要（工具名 + description + 耗时）的就地渲染与 Task 去重。

#### Scenario: 子代理卡片就地渲染（includeSubagents=true）

- **WHEN** session 包含 subagent processes 且 `includeSubagents` 为 true
- **THEN** 子代理卡片摘要（工具名 + description + 耗时）SHALL 按 spawn 时间穿插渲染在父 turn 对应位置
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 被去重不重复渲染

#### Scenario: 关闭子代理仅保留发起工具（includeSubagents=false）

- **WHEN** `includeSubagents` 为 false
- **THEN** 导出 SHALL NOT 渲染子代理卡片
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 作为普通工具调用正常渲染（不被去重丢失）

### Requirement: 导出对话流时序

Markdown / HTML 导出的 Assistant turn 内，thinking / 正文文本 / 工具调用 / subagent 卡片 SHALL 按时间顺序穿插渲染，与 SessionDetail 视图所见顺序一致。导出器 MUST 复用 SessionDetail 视图的同一时序合并实现（`displayItemBuilder.ts::buildDisplayItems`），SHALL NOT 在导出器内另写一套排序，也 SHALL NOT 把工具调用 / subagent 统一堆到 turn 末尾。最终 assistant 文本（`buildDisplayItems` 抽出的 `lastOutput`）SHALL 渲染在该 turn 其余 item 之后，与视图布局一致。

#### Scenario: 工具调用穿插在文本之间

- **WHEN** 一个 Assistant turn 含「文本 A → 工具调用 T → 文本 B（最终输出）」的时序
- **THEN** 导出内容 SHALL 按 文本 A、工具调用 T、文本 B 的顺序渲染
- **AND** 工具调用 T SHALL NOT 出现在文本 B 之后

#### Scenario: subagent 卡片按 spawn 时间穿插

- **WHEN** 一个 Assistant turn 在两段文本之间 spawn 了一个 subagent
- **THEN** 导出内容中 subagent 卡片 SHALL 出现在对应 spawn 时间点的位置
- **AND** SHALL NOT 被统一追加到 turn 末尾

#### Scenario: 导出顺序与视图一致

- **WHEN** 同一 session 在 SessionDetail 视图渲染并导出为 Markdown
- **THEN** 导出件中 thinking / 文本 / 工具 / subagent 的相对顺序 SHALL 与视图 DisplayItem（含末尾 `lastOutput`）顺序一致

### Requirement: 导出数据完整性

导出（Markdown / JSON / HTML）SHALL 基于保留 tool output 与 response content 的 SessionDetail 生成，SHALL NOT 复用首屏被 `outputOmitted` / `contentOmitted` 裁剪过的 payload。导出 MUST 通过导出专用数据路径（桌面端 `get_session_detail_for_export`，浏览器模式复用 HTTP `get_session_detail`）拉取 SessionDetail，使工具 output 与响应内容字段与源 JSONL 一致。当 `toolOutputMode` 为 `full` 时工具 output SHALL 完整非空。image data 与 subagent messages 在导出路径仍被裁剪以控制 payload。

#### Scenario: full 模式工具 output 非空

- **WHEN** 用户以 `toolOutputMode = full` 导出含工具调用（且源数据有 output）的会话
- **THEN** 导出件中每个工具调用 SHALL 渲染其真实 output 内容
- **AND** output SHALL NOT 为空（除非源 JSONL 本就无 output）

#### Scenario: JSON 导出保留工具 output 与 response content

- **WHEN** 用户导出为 JSON
- **THEN** 序列化结果中 `tool_executions[].output` SHALL 含真实内容
- **AND** `responses[].content` SHALL NOT 为空串且 `contentOmitted` SHALL NOT 为 true

#### Scenario: 桌面与浏览器导出 tool output 一致

- **WHEN** 同一会话分别在 Tauri 桌面端与浏览器 HTTP 模式导出
- **THEN** 两者工具调用的 output 与 response content SHALL 一致非空

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


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

当 session 包含子代理（subagent / workflow）时，导出 SHALL 包含其内容摘要或完整对话。

#### Scenario: 子代理内容展开

- **WHEN** session 包含 subagent processes
- **AND** 导出选项 includeSubagents 为 true
- **THEN** 子代理的对话内容作为嵌套结构包含在父 turn 内
- **AND** Markdown 中用缩进或引用块表示层级
- **AND** HTML 中用可折叠的嵌套区域展示

#### Scenario: 子代理内容折叠

- **WHEN** includeSubagents 为 false
- **THEN** 仅显示子代理摘要（工具名 + description + 耗时）
- **AND** 不包含子代理内部的对话详情


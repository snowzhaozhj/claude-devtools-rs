# tool-viewer-routing Specification

## Purpose

让用户展开任意工具调用时立即看到该工具最相关的信息：Read 调用一眼能看到读了哪个文件 / 哪几行 / 内容；Edit 调用看到改了哪几行；Bash 调用看到命令与输出；其它工具回退到通用展示。配套显示工具耗时、等待状态、失败原因；输出量大时不阻塞 UI 交互。删了这个 capability，用户展开工具时会看到一团原始 JSON、不知工具是否完成 / 失败、且大输出会卡住整个会话页面。本 capability 同时覆盖主会话工具列表与 SubagentCard 内嵌套的子调用 trace。
## Requirements
### Requirement: 工具专化查看器路由

展开的工具项 SHALL 根据 toolName 路由到专化查看器。未知工具 SHALL 使用默认查看器。每个工具查看器 SHALL 在 header 区域提供 inline copy 按钮用于一键复制工具内容。

#### Scenario: Read 工具
- **WHEN** toolName 为 "Read" 且无错误
- **THEN** SHALL 使用 ReadToolViewer 渲染（显示文件路径、行号范围、代码内容）
- **AND** header SHALL 包含 copy 按钮，点击复制文件内容（strip 行号前缀后的纯文本）

#### Scenario: Edit 工具
- **WHEN** toolName 为 "Edit"
- **THEN** SHALL 使用 EditToolViewer 渲染（显示 old_string / new_string 对比）

#### Scenario: Write 工具
- **WHEN** toolName 为 "Write" 且无错误
- **THEN** SHALL 使用 WriteToolViewer 渲染（显示文件路径和内容）
- **AND** header SHALL 包含 copy 按钮，点击复制文件写入内容全文

#### Scenario: Bash 工具
- **WHEN** toolName 为 "Bash" 或 "bash"
- **THEN** SHALL 使用 BashToolViewer 渲染（显示命令和输出）
- **AND** header SHALL 包含 copy 按钮，点击复制命令输出文本

#### Scenario: 未知工具
- **WHEN** toolName 不匹配任何专化查看器
- **THEN** SHALL 使用 DefaultToolViewer 渲染（显示 JSON 输入和文本输出）

### Requirement: Lazy load tool output on expand

ExecutionTrace 渲染的每个 tool item SHALL 在 `toggle(key)` 展开时检查对应 `exec.outputOmitted`：若为 `true` 且本地 `outputCache` 未命中，SHALL 调 IPC `getToolOutput(rootSessionId, sessionId, toolUseId)` 拉取 `ToolOutput` 并写入本地缓存；ToolViewer 渲染 SHALL 优先用本地 `outputCache.get(toolUseId)`，fallback 到 `exec.output`（兼容 `outputOmitted=false` 老后端 / 回滚路径）。本机制对主 SessionDetail 与 SubagentCard 内嵌套 ExecutionTrace 同等适用，sessionId 参数 SHALL 由所在 trace 的 sessionId 提供（嵌套 subagent trace 用 subagent 的 sessionId）。

#### Scenario: 折叠状态不触发 IPC

- **WHEN** SessionDetail 首屏渲染含 N 个 tool execution，且 `outputOmitted=true`
- **THEN** 前端 SHALL NOT 调 `getToolOutput`，仅渲染 BaseItem header（label / summary / status）
- **AND** Network 面板 SHALL 显示 0 次 `get_tool_output` 调用

#### Scenario: 展开时按需拉

- **WHEN** 用户点击某个 tool item 触发 `toggle(key)`
- **AND** 对应 `exec.outputOmitted=true` 且本地 `outputCache` 未命中
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, sessionId, exec.toolUseId)` 一次
- **AND** 拉取成功后 SHALL 把结果写入本地 `outputCache.set(toolUseId, output)`，触发 ToolViewer 用新 output 重渲染

#### Scenario: 重复展开复用本地缓存

- **WHEN** 用户先展开后折叠再展开同一 tool item
- **THEN** 第二次展开 SHALL NOT 触发 `getToolOutput` IPC（直接用 `outputCache.get(toolUseId)`）

#### Scenario: 老后端 / 回滚开关 fallback

- **WHEN** 后端响应中 `outputOmitted=false` 或字段缺失（老后端）
- **THEN** 前端 SHALL 直接渲染 `exec.output`，SHALL NOT 调 `getToolOutput`

#### Scenario: 嵌套 subagent 内 tool 用 subagent sessionId 拉

- **WHEN** SubagentCard 展开后渲染嵌套 ExecutionTrace，用户点击其中某 tool
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, subagent.sessionId, toolUseId)`，sessionId 用 subagent 的，不复用 root 的

#### Scenario: IPC 失败不阻塞 UI

- **WHEN** `getToolOutput` IPC 抛错或返回 `ToolOutput::Missing`
- **THEN** 前端 SHALL 渲染 fallback 显示（如"output 加载失败"或空状态），SHALL NOT 阻塞其它 tool item 的展开

### Requirement: Tool row displays approximate token count

ExecutionTrace 与 AI chunk 内联工具渲染中，每个工具 row 的 `BaseItem` MUST 通过 `getToolContextTokens(exec)` 估算 token 总和并以 `~{formatTokens(N)} tokens` 文案显示，与原版 `BaseItem.tsx:150` 格式一致。估算算法 MUST 为：

- input 部分：`estimateContentTokens(exec.input)`——对象/数组先 `JSON.stringify` 后按 ~4 字符/token 启发式计算
- output 部分：按 `ToolOutput.kind` 分支——`text` 取 `text` 字段走 `estimateTokens`；`structured` 取 `value` 走 `estimateContentTokens`；`missing` 贡献 0

`~N` 数字槽 SHALL 在 status 圆点之前渲染；工具 row 同时 MUST 在 status 圆点之后显示 `durationMs`（如 `25ms`）。当 `getToolContextTokens` 返回 0（空 input + missing output）时 row SHALL 不显示 token 槽。

#### Scenario: Bash 工具 row 显示 token 与 duration
- **WHEN** 一条 Bash tool row 的 `input={command: "ls -la"}` + `output.kind="text"` + `output.text="foo.rs\nbar.rs\n..."` 约 200 字符，duration 25ms
- **THEN** row MUST 显示 `~50 tokens` 槽（`ceil((len(JSON.stringify(input)) + 200) / 4)`）+ status 圆点 + `25ms`

#### Scenario: missing output 工具仍显示 input token
- **WHEN** 工具 `output.kind="missing"`（IPC 懒裁剪前的初始状态），`input={file_path: "/tmp/x.txt"}` JSON 约 40 字符
- **THEN** row MUST 显示 `~10 tokens`（仅 input 部分，output 贡献 0）

### Requirement: 大文本工具详情交互优先渲染

Read、Write、Edit 工具详情在展开较大文本内容时 SHALL 避免一次性对所有行执行重型同步语法高亮或 HTML 清洗；小/中等 Read、Write 内容 SHALL 保留完整语法高亮，大文本才 SHALL 降级到轻量高亮。展开交互 MUST 先让 header、容器、滚动和点击目标保持可响应。任何通过 `{@html}` 注入的工具内容 MUST 来自受控内部渲染器输出或经过 XSS 防护清洗。`outputOmitted=true` 时，前端 SHALL 按工具实际路由的 viewer 决定是否先 IPC 拉取输出再展开：路由到 ReadToolViewer / BashToolViewer / DefaultToolViewer 的工具（含 `isError=true` 的 Read 与 Write —— 这两类走 DefaultToolViewer 渲染错误详情）依赖 `exec.output`，SHALL 先拉到再展开；路由到 EditToolViewer 的 Edit 工具（任意 `isError`）与路由到 WriteToolViewer 的 `isError=false` 的 Write 工具仅渲染 `exec.input` 字段，SHALL 立即展开，不为不会被消费的 output 引入额外延迟。

#### Scenario: Read 大文本展开不阻塞整页交互
- **WHEN** 用户展开一个 Read 工具项，且该工具输出包含数百行文本
- **THEN** 工具详情 SHALL 渲染路径、行号容器和文件内容
- **AND** SHALL 使用轻量高亮，SHALL NOT 对所有行同步执行 `highlight.js` + `DOMPurify` 后才允许交互

#### Scenario: Read 小/中等文本保留完整语法高亮
- **WHEN** 用户展开一个 Read 工具项，且该工具输出未达到大文本阈值
- **THEN** 代码行 SHALL 保留 `highlight.js` 语法高亮

#### Scenario: Write 大文本展开不阻塞整页交互
- **WHEN** 用户展开一个 Write 工具项，且输入内容包含数百行文本
- **THEN** 工具详情 SHALL 渲染文件路径和文件内容
- **AND** SHALL 使用轻量高亮，SHALL NOT 对所有行同步执行 `highlight.js` + `DOMPurify` 后才允许交互

#### Scenario: Write 小/中等文本保留完整语法高亮
- **WHEN** 用户展开一个 Write 工具项，且输入内容未达到大文本阈值
- **THEN** 代码行 SHALL 保留 `highlight.js` 语法高亮

#### Scenario: Edit diff 行不做重型语法高亮
- **WHEN** 用户展开一个 Edit 工具项，且 diff 包含多行 added、removed 或 context 内容
- **THEN** DiffViewer SHALL 保留统一 diff 结构、old/new 行号、增删背景与 `+`/`-` 前缀
- **AND** SHALL NOT 对每个 diff 行执行 `highlight.js` 语法高亮

#### Scenario: 工具详情 HTML 注入保持安全边界
- **WHEN** Read、Write 或 Edit 工具内容包含类似 HTML 或脚本片段的文本
- **THEN** 渲染结果 MUST 将其作为代码/文本展示，SHALL NOT 执行脚本或注入未清洗 HTML

#### Scenario: omitted output 工具输出 ready 后再展开
- **WHEN** 用户首次展开一个 `outputOmitted=true` 且尚未缓存输出的工具项，且该工具的查看器使用 `exec.output` 渲染（Read / Bash / DefaultToolViewer 路径）
- **THEN** SessionDetail SHALL 先调 `getToolOutput(rootSessionId, sessionId, exec.toolUseId)` 拉取完整输出
- **AND** SHALL 在输出可用后再把该工具项加入展开集合，避免空 OUTPUT 区域被实际内容跳变替换
- **AND** IPC 失败或返回 `kind = "missing"` 时 SHALL 保持工具项折叠，让用户重试

#### Scenario: 嵌套 ExecutionTrace 的 omitted output 工具输出 ready 后再展开
- **WHEN** 用户在 SubagentCard 的 ExecutionTrace 中首次展开一个 `outputOmitted=true` 且尚未缓存输出的工具项，且该工具的查看器使用 `exec.output` 渲染（Read / Bash / DefaultToolViewer 路径）
- **THEN** ExecutionTrace SHALL 先调 `getToolOutput(rootSessionId, traceSessionId, exec.toolUseId)` 拉取完整输出
- **AND** SHALL 在输出可用后再把该工具项加入展开集合
- **AND** `traceSessionId` SHALL 为该 trace 所属 session 的 sessionId（嵌套 subagent 时为 subagent 自己的 sessionId）

#### Scenario: 不依赖 output 的工具立即展开
- **WHEN** 用户展开一个 Edit 工具项（`isError` 任意），或 `isError=false` 的 Write 工具项
- **THEN** SessionDetail 与 ExecutionTrace SHALL 立即把该工具项加入展开集合，SHALL NOT 等待 `getToolOutput` IPC
- **AND** 渲染 SHALL 仅依赖 `exec.input` 字段（Edit 的 `old_string` / `new_string`、Write 的 `content`），与 `exec.output` 无关
- **AND** `isError=true` 的 Write 与 Read 工具项 SHALL NOT 走本场景，而是落到上一条 "omitted output 工具输出 ready 后再展开" Scenario —— 它们走 DefaultToolViewer 渲染错误详情，需要 `exec.output`

#### Scenario: 展开 AIChunk 不主动 prefetch Bash 与 Default
- **WHEN** 用户点击 AIChunk header 把工具区域展开
- **THEN** SessionDetail SHALL NOT 对该 chunk 内的 Bash 或 Default 路径工具触发 `getToolOutput` IPC（`prefetchReadOutputs` 仅命中 Read）
- **AND** Bash / Default 工具的 output 拉取 SHALL 仅在用户点击该具体工具项展开时按需触发，避免一次 chunk 展开引起并发 IPC 把"展开工具列表"交互拖慢

#### Scenario: 工具详情展开状态局部更新
- **WHEN** 用户展开或收起单个工具项
- **THEN** SessionDetail SHALL 保持其他 chunk 与工具项的展开状态不变
- **AND** SHALL 避免因该单项状态变化重新执行与该工具无关的 Markdown、Mermaid 或 diff 渲染工作

### Requirement: Tool detail timing and failure visibility

SessionDetail SHALL 在所有工具明细展示路径中显示可用的时间统计与失败原因。该规则适用于主会话工具列表和 subagent ExecutionTrace 内的工具项。

#### Scenario: Completed tool shows duration

- **WHEN** 一个工具执行同时具有 `startTs` 与 `endTs`
- **THEN** 工具明细 Header SHALL 显示由二者差值格式化得到的耗时

#### Scenario: Pending tool shows waiting state

- **WHEN** 一个工具执行具有 `startTs` 但缺少 `endTs`
- **THEN** 工具明细 Header SHALL 显示等待或进行中状态，而不是空白时间统计

#### Scenario: Failed tool shows readable reason

- **WHEN** 一个工具执行 `isError=true` 且 `output` 含文本或结构化错误内容
- **THEN** 展开工具明细 SHALL 显示失败原因
- **AND** 失败原因 SHALL 保留 raw 文本或格式化 JSON fallback，避免只显示失败状态

#### Scenario: Subagent trace tool uses same metadata display

- **WHEN** subagent ExecutionTrace 中渲染一个工具项
- **THEN** 该工具项 SHALL 使用与主会话工具项相同的耗时、等待状态与失败原因展示规则

### Requirement: Tool result expansion avoids eager heavy rendering

工具调用结果 SHALL 只在用户展开对应工具项后渲染重内容；重复展开同一工具项 SHALL 复用已计算的渲染结果。大型 markdown、代码高亮或 JSON 输出 SHALL 遵循 lazy 渲染策略，避免折叠状态和首次展开时造成明显主线程卡顿。

#### Scenario: Collapsed tool does not render heavy output

- **WHEN** 一个工具项处于折叠状态且 output 很大
- **THEN** SessionDetail SHALL NOT 为该 output 执行 markdown 渲染、语法高亮或大 JSON DOM 构建

#### Scenario: First expansion renders on demand

- **WHEN** 用户首次展开该工具项
- **THEN** 工具详情 SHALL 渲染可见内容
- **AND** 大型文本 SHALL 继续使用 lazy markdown 或等价的分帧/视口触发机制

#### Scenario: Re-expansion reuses cached render result

- **WHEN** 用户展开工具项、折叠后再次展开同一工具项
- **THEN** UI SHALL 复用已缓存的渲染结果或派生数据，避免重复执行昂贵转换

### Requirement: Workflow tool summary 生成

getToolSummary SHALL 为 Workflow 工具生成人类可读的 header 摘要文本。

#### Scenario: Workflow tool summary
- **WHEN** toolName 为 "Workflow"
- **THEN** SHALL 优先显示 input.name（截断到 50 字符）
- **AND** 若 name 缺失但 run_id/runId 存在 SHALL 显示 "run {runId}"（截断到 20 字符）
- **AND** 若两者都缺失 SHALL 显示 "Workflow"

### Requirement: 工具查看器按内容规模自适应展示

工具专化查看器（Read / Write / Bash / Default 查看器与 diff 展示，见 `[[edit-diff-view]]`）在展示工具内容时 SHALL 按内容规模自适应，统一到"完整内联 / 限高预览 / 首尾切片"三级语义，替代各查看器各自固定像素限高的隐式行为。工具类型 SHALL 只决定内容如何渲染（代码高亮 / diff / 纯文本），SHALL NOT 决定信息是否完整可得。规模阈值 SHALL 复用 `[[session-display]]` 的 `对话流输出自适应展示的规模阈值` Requirement（限高档 80 行或 16 KiB；超大档 1000 行或 256 KiB；UTF-8 字节度量；`>=` 升档）。

**分档所依据的内容面**（权威数据源矩阵）SHALL 为该查看器实际渲染给用户的主内容，而非固定取某一字段：

- Read / Bash 查看器、以及走 Default 查看器渲染错误详情的失败工具：依据工具输出内容。
- Write 查看器：依据待写入文件内容（工具输入的文件内容字段），其输出通常仅为成功回执，SHALL NOT 用回执规模判定。
- Edit 查看器 / diff 展示：依据 old / new 差异内容（工具输入），其输出通常为 Missing，分档 SHALL NOT 依赖输出。
- Default 查看器：依据输出内容；失败态附带的错误信息文本 SHALL 一并计入规模。

三档展示：

- **完整内联**：低于限高阈值时 SHALL 完整渲染，无竖向内部滚动、无预览提示。
- **限高预览**：中长内容 SHALL 在响应式限高的内部滚动区域内完整渲染，并显示信息气味（总行数与总字节数 + "预览"状态）；竖向滚动区域 SHALL 使用稳定滚动槽避免懒加载 / 展开 / 窗口缩放时的横向跳变。
- **首尾切片**：超大内容 SHALL 只渲染首部与尾部切片，二者之间 SHALL 插入省略接缝显式标注被省略的行数或字节数，SHALL NOT 一次性构建超大内容的完整 DOM，也 SHALL NOT 用渐隐遮罩暗示截断。首尾切片 SHALL 仅用于行导向的纯文本 / 代码 / diff 内容（按行切分语义安全）；markdown 富文本内容 SHALL NOT 首尾切片，改用限高预览（见 `[[session-display]]`）。

该自适应展示 SHALL 与既有 `大文本工具详情交互优先渲染` 的轻量高亮降级、`Tool result expansion avoids eager heavy rendering` 的懒渲染策略叠加生效，SHALL NOT 削弱这两条既有约束。本 change 不引入应用内查看超大内容完整中段的能力（需后端分段通道）；超大工具内容的完整获取路径 SHALL 为复制全文（见 `[[copy-to-clipboard]]`）。

#### Scenario: 短工具输出完整内联

- **WHEN** 用户展开一个主内容行数与字节数均低于限高阈值的工具项
- **THEN** 内容 SHALL 完整渲染
- **AND** SHALL NOT 出现竖向内部滚动条与预览提示

#### Scenario: 中长工具输出限高预览带信息气味

- **WHEN** 用户展开一个主内容超过限高档未达超大档的工具项
- **THEN** 内容 SHALL 在响应式限高的内部滚动区域内完整渲染
- **AND** SHALL 显示总行数与总字节数及"预览"状态
- **AND** 竖向滚动区域 SHALL 使用稳定滚动槽

#### Scenario: 超大行导向输出首尾切片

- **WHEN** 用户展开一个主内容达到超大档、且为行导向纯文本 / 代码 / diff 的工具项
- **THEN** SHALL 只渲染首尾切片 + 标注省略量的省略接缝
- **AND** SHALL NOT 一次性构建超大内容的完整 DOM

#### Scenario: 写入型工具按输入内容规模分档

- **WHEN** 一个成功的写文件工具其待写入内容达到超大档、而输出仅为简短成功回执
- **THEN** 分档 SHALL 依据待写入内容规模，进入超大档
- **AND** SHALL NOT 因回执很小而被判为完整内联档

#### Scenario: 编辑型工具无输出时按差异内容分档

- **WHEN** 一个成功的编辑工具其 old / new 差异达到限高或超大档、而输出为 Missing
- **THEN** 分档 SHALL 依据差异内容规模
- **AND** SHALL NOT 因输出缺失而被判为完整内联档

### Requirement: 工具输出懒加载态的稳定分档

工具输出经首屏裁剪（`outputOmitted=true`）后按需懒加载。为使懒加载前后的展示不违反 `[[session-display]]` 的滚动稳定契约，工具查看器输出的分档 SHALL 遵循确定的状态转换：

- **规模信号优先级**：已加载的真实内容规模 > 裁剪层记录的 `outputBytes` > 未知。分档 SHALL 使用当前可得的最高优先级信号。
- **裁剪空值不等于零规模**：`outputOmitted=true` 时被清空的内容占位（空字符串 / Null）SHALL NOT 被当作 0 字节判入完整内联档。
- **规模未知时按需先取**：当内容尚未加载且 `outputBytes` 缺失（老后端 / 解析层未填）时，展开该工具 SHALL 先触发懒加载，展示层 SHALL 以稳定的加载占位渲染（占位高度等于限高档的内部滚动区域高度），SHALL NOT 先按空内容判入完整内联档再在加载后跳变。
- **加载后校正不放大外层几何**：懒加载到达后按真实内容规模确定最终档位；最终档为限高 / 超大时，内容 SHALL 在同一稳定的外层限高 viewport 内填充，外层 viewport 的占位几何 SHALL NOT 在加载前后改变（bounded 占位 → bounded / oversized 最终态零跳变）。最终档为完整内联时，viewport MAY 收缩为内容自然高度——完整内联内容保留限高档空白占位反而制造持续的视觉噪音；该收缩发生于用户展开交互点，或伴随 tab 恢复的锚点滚动恢复机制，不视为滚动稳定契约破坏（见 design D6b 取舍记录）。

#### Scenario: outputBytes 缺失时先加载再分档

- **WHEN** 用户展开一个 `outputOmitted=true`、`outputBytes` 缺失、尚未缓存输出的工具项
- **THEN** SHALL 先触发输出懒加载
- **AND** 加载期间 SHALL 以稳定的加载占位（限高档高度）渲染，不判入完整内联档
- **AND** 加载到达后 SHALL 按真实内容规模确定最终档位

#### Scenario: 裁剪空值不被判为短内容

- **WHEN** 一个工具项 `outputOmitted=true`，其被清空的输出占位为空字符串
- **THEN** SHALL NOT 因占位为空而判入完整内联档
- **AND** SHALL 以 `outputBytes`（若有）或先加载后校正的方式分档

#### Scenario: 预估短档、实际超大档时外层几何不跳变

- **WHEN** 一个恢复展开状态的工具项，其 `outputBytes` 估算落在短档、但懒加载到的真实内容达到超大档
- **AND** 该工具项位于当前视口上方
- **THEN** 外层限高 viewport 的占位几何 SHALL 在加载前后保持不变
- **AND** 用户当前阅读位置 SHALL NOT 出现可感知跳动

### Requirement: 首尾切片的渲染上限与切分安全

工具查看器超大档的首尾切片 SHALL 有可验证的渲染上限与切分规则，使"只渲染切片"不被实现成接近完整 DOM，且不产生截断的字符或重复内容：

- **每侧上限**：首部与尾部各自 SHALL 有明确的最大渲染行数与最大渲染字节数上限，任一先达到即停止该侧切片增长。
- **重叠规避**：当内容总行数不超过首尾两侧上限之和时，SHALL NOT 切片——SHALL 退回限高预览完整渲染，避免首尾重叠导致同段内容重复展示。
- **字符安全**：切分点 SHALL 落在 Unicode 码点与行边界上，SHALL NOT 在多字节字符中间截断产生非法字节序列。组合字符簇（ZWJ emoji / 组合变音符）的完整性为非目标——超大单行按字节预算切分时簇可能被拆为可见的组成码点，但 SHALL NOT 产生乱码（见 design D7b 取舍记录）。
- **省略量**：省略接缝标注的省略行数 / 字节数 SHALL 等于总量减去首尾两片实际渲染量，SHALL NOT 少算或把重叠部分重复计入。

#### Scenario: 无换行超大单行不整行重复渲染

- **WHEN** 一个工具输出是单行、无换行、字节数远超超大档边界的内容
- **THEN** 首尾切片 SHALL 按每侧字节上限截取，且切分点落在完整字符边界
- **AND** SHALL NOT 把整行渲染为首片又渲染为尾片

#### Scenario: 行数不足以切片时退回限高预览

- **WHEN** 一个内容行数超过超大档边界、但不超过首尾两侧最大渲染行数之和
- **THEN** SHALL NOT 首尾切片
- **AND** SHALL 退回限高预览完整渲染

#### Scenario: 省略量等于被省略的真实量

- **WHEN** 一个超大内容被首尾切片
- **THEN** 省略接缝标注的省略量 SHALL 等于总量减去首尾两片实际渲染量

### Requirement: 工具查看器内部滚动键盘可访问

工具查看器的限高内部滚动区域 SHALL 遵循与 `[[session-display]]` 的 `输出内部滚动区域键盘可访问` Requirement 相同的规则：沿任一轴实际溢出时作为可聚焦滚动 viewport 进入 Tab 序列、未溢出不增加该 viewport 的 Tab 停靠点、头部动作控件独立键盘可达、提供含工具名与内容规模的可访问名称、保留边界滚动链。工具项收起后其内部滚动 viewport SHALL 退出 Tab 序列。

#### Scenario: 溢出的工具输出可键盘滚动

- **WHEN** 用户展开的工具项主内容沿任一轴实际溢出其限高区域
- **THEN** 该滚动 viewport SHALL 可通过键盘 Tab 进入并用方向键 / Page 键滚动
- **AND** SHALL 提供含工具名与内容规模的可访问名称

#### Scenario: 收起的工具输出退出 Tab 序列

- **WHEN** 用户收起一个此前其内部滚动 viewport 可键盘聚焦的工具项
- **THEN** 该内部滚动 viewport SHALL NOT 再出现在键盘 Tab 序列中


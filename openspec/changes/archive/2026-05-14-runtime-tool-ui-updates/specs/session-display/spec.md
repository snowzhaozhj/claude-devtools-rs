## ADDED Requirements

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

### Requirement: Edit diff preview highlighting

Edit 工具展开内容 SHALL 保持统一 diff 视图，并根据 `file_path` 推断语言后对 diff 行内容进行语法高亮；无法推断或高亮失败时 MUST 降级为纯文本 diff，不影响 added/removed/context 样式和行号。

#### Scenario: Edit diff highlights by file extension

- **WHEN** Edit 工具 input 含 `file_path="src/lib.rs"`、`old_string` 与 `new_string`
- **THEN** DiffViewer SHALL 以 Rust 语言规则高亮 diff 行内容
- **AND** added/removed/context 背景、前缀与 old/new 双列行号 SHALL 保持可见

#### Scenario: Unknown extension falls back to plain diff

- **WHEN** Edit 工具 input 的文件扩展名无法映射到高亮语言
- **THEN** DiffViewer SHALL 渲染纯文本 diff
- **AND** SHALL NOT 抛错或显示空白预览

#### Scenario: Pure insert still previews content

- **WHEN** Edit 工具只有 `new_string` 或 `old_string` 为空
- **THEN** DiffViewer SHALL 显示所有新增行并应用可用的语言高亮

#### Scenario: Trailing newline does not create phantom diff row

- **WHEN** Edit 工具对比 `old_string="a\n"` 与 `new_string="b\n"`
- **THEN** DiffViewer SHALL 只显示一条 removed `a` 与一条 added `b`
- **AND** SHALL NOT 显示额外空白 context 行

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

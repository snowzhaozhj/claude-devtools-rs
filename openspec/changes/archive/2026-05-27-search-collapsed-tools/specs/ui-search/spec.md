## MODIFIED Requirements

### Requirement: 会话内文本搜索与高亮

输入搜索文本后，系统 SHALL 在 conversation 容器的可见文本节点中查找所有匹配项，并通过 `<mark>` 元素高亮显示。搜索 SHALL 大小写不敏感。此外，系统 SHALL 对折叠状态的 AI chunk 执行数据层虚拟匹配：遍历每个未展开 AI chunk 的 `toolExecutions[]`，对 toolName 和 tool summary 做 case-insensitive 子串匹配，产出虚拟匹配项追加到 DOM 匹配列表之后，使搜索总数包含折叠工具名命中。

#### Scenario: 输入搜索文本
- **WHEN** 用户在搜索框中输入文本（300ms debounce 后）
- **THEN** conversation 容器中所有匹配的文本片段 SHALL 被 `<mark>` 元素包裹高亮

#### Scenario: 大小写不敏感
- **WHEN** 搜索文本为 "error" 且内容中包含 "Error"、"ERROR"
- **THEN** 所有变体 SHALL 均被高亮

#### Scenario: 跳过代码块
- **WHEN** 搜索执行时
- **THEN** `<pre>`、`<code>`、`<style>`、`<script>` 标签内的文本 SHALL 不参与匹配

#### Scenario: 无匹配结果
- **WHEN** 搜索文本在 conversation 中无匹配（DOM 层和虚拟匹配均为零）
- **THEN** 搜索栏 SHALL 显示 "无结果"

#### Scenario: 折叠 AI chunk 工具名参与搜索
- **WHEN** 搜索文本为 "Read" 且某个未展开 AI chunk 的 toolExecutions 中包含 toolName "Read"
- **THEN** 搜索计数 SHALL 包含该虚拟匹配项
- **AND** 搜索栏显示的总数 SHALL 为 DOM 匹配数 + 虚拟匹配数

#### Scenario: 折叠工具 summary 参与搜索
- **WHEN** 搜索文本为 "config.ts" 且某个未展开 AI chunk 的 toolExecution 的 getToolSummary 结果包含 "config.ts"
- **THEN** 搜索计数 SHALL 包含该虚拟匹配项

#### Scenario: 导航到虚拟匹配时按需展开
- **WHEN** 用户通过 Enter/导航按钮导航到一个虚拟匹配项
- **THEN** 系统 SHALL 展开对应 AI chunk 的工具区域
- **AND** SHALL 定位到对应 `[data-tool-use-id]` 元素并滚动到视口中心
- **AND** 展开后 SHALL 重搜以去重（虚拟匹配变为 DOM 匹配）

#### Scenario: 已展开 chunk 的工具名不产生虚拟匹配
- **WHEN** AI chunk 已展开（工具区域在 DOM 中）
- **THEN** 该 chunk 的工具名 SHALL 仅通过 DOM 层搜索匹配，MUST NOT 产生额外虚拟匹配（避免重复计数）

#### Scenario: 关闭搜索不恢复折叠状态
- **WHEN** 搜索导航导致某 chunk 被展开后用户关闭搜索栏
- **THEN** 被展开的 chunk SHALL 保持展开状态（sticky）

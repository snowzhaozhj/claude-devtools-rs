## MODIFIED Requirements

### Requirement: SemanticStep 渲染

前端 SHALL 在 SessionDetail 的 semantic steps 遍历中，对 `kind === "user_message"` 的 step 渲染一个 BaseItem disclosure 行：
- `svgIcon` = `MESSAGE_SQUARE`（与 Output 行同 icon）
- `label` = `"User"`
- `summary` = 消息文本截断（超 60 字符时 `text.slice(0, 60) + "…"`）
- 可展开查看完整文本（markdown 渲染）
- 无 tokenCount
- 无状态标记（无 ✓/✗）

该行 SHALL 出现在 semantic steps 序列的精确位置（后端已按时序排列），前端按顺序渲染即可。

#### Scenario: Short user message rendered inline
- **WHEN** AIChunk.semanticSteps 含 `{ kind: "user_message", text: "短文本" }`
- **THEN** 渲染 BaseItem：icon=MESSAGE_SQUARE, label="User", summary="短文本", 无展开内容

#### Scenario: Long user message truncated with expand
- **WHEN** AIChunk.semanticSteps 含 `{ kind: "user_message", text: "超过60字符的长文本..." }`
- **THEN** 渲染 BaseItem：summary 截断 60 字符 + "…"，点击展开显示完整 markdown

#### Scenario: Unknown step kind is silently skipped
- **WHEN** 老版前端遇到未识别的 step kind（如 `"user_message"` 在不支持的版本）
- **THEN** 该 step 不渲染，不报错（{#each} 无匹配分支自然跳过）

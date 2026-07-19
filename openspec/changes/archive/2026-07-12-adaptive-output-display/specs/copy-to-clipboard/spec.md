## RENAMED Requirements

- FROM: `### Requirement: OutputBlock 代码块提供 overlay copy`
- TO: `### Requirement: 输出块提供常驻可发现的复制全文入口`

## MODIFIED Requirements

### Requirement: 输出块提供常驻可发现的复制全文入口

承载代码 / 命令 / 工具输出的输出块 SHALL 在其头部区域提供**常驻可发现**的复制全文入口，SHALL NOT 仅在鼠标悬停时出现。复制入口 SHALL 使用可访问的按钮语义与键盘可达性。当完整原文可得时，复制内容 SHALL 为该输出面的**完整原文**，SHALL NOT 仅为当前可见的限高预览或首尾切片文本。

各输出面的"完整原文"来源：AI 文本输出为其完整文本；读取 / 命令 / Default 工具为完整工具输出；写入工具为完整待写入内容；编辑 / diff 为其完整差异文本。

复制入口 SHALL 覆盖完整原文尚不可得的状态，且 SHALL NOT 降级为复制可见片段：

- 完整原文正在懒加载、或加载失败、或为缺失 / 空内容时，复制入口 SHALL 处于禁用态（保留可读标签或 tooltip 说明原因），SHALL NOT 改为复制当前可见的预览或切片。
- 空内容的复制（若允许）SHALL 明确产生空结果，SHALL NOT 伪装成成功复制了内容。

复制成功与失败的反馈 SHALL 沿用既有 `点击 CopyButton 复制文本并显示反馈` Requirement 的语义（成功切换图标、失败静默降级不改按钮态），本 Requirement SHALL NOT 引入与之冲突的新失败反馈；本 Requirement 仅新增"未就绪时禁用、且任何情况下不复制可见片段"的约束。

#### Scenario: 复制入口常驻可发现

- **WHEN** 输出块渲染（任意规模档）
- **THEN** 头部区域 SHALL 显示复制全文入口
- **AND** 该入口 SHALL NOT 依赖鼠标悬停才可见
- **AND** SHALL 可通过键盘聚焦与触发

#### Scenario: 复制完整原文而非可见片段

- **WHEN** 输出块处于限高预览或首尾切片状态、完整原文已可得、用户点击复制
- **THEN** SHALL 复制该输出面的完整原文
- **AND** SHALL NOT 仅复制当前可见的预览或切片文本

#### Scenario: 完整原文未就绪时复制入口禁用

- **WHEN** 输出块的完整原文正在懒加载、或加载失败、或为缺失内容
- **THEN** 复制全文入口 SHALL 处于禁用态并说明原因
- **AND** SHALL NOT 降级为复制当前可见的预览或切片

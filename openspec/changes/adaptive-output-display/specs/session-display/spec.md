## ADDED Requirements

### Requirement: 对话流文本输出按内容规模自适应展示

SessionDetail 对话流中的 markdown 文本输出块——AIChunk 内的输出（AI 文本步骤）与用户消息展示项、AIChunk 末尾常显的最后输出，以及 subagent / workflow 卡片内嵌执行轨迹中的对应文本输出——SHALL 按内容规模自适应展示，使短内容零摩擦、长内容不淹没对话流。规模判定 SHALL 基于内容的原始行数与字节数（见 `对话流输出自适应展示的规模阈值` Requirement），SHALL NOT 依据渲染后 DOM 高度反推，以保持与 `按 Chunk 类型渲染对话流` 的滚动稳定契约一致（不引入"离屏估算高度占位、进视口换真高度"机制）。

两档展示：

- **完整内联**：行数与字节数均低于限高阈值时，SHALL 完整渲染，无竖向内部滚动、无"预览"提示。
- **限高预览**：达到限高阈值时，SHALL 在响应式限高的内部滚动区域内渲染**完整内容**，并显示信息气味（总行数与总字节数 + "预览"状态）。

markdown 文本输出 SHALL NOT 采用只渲染首尾、省略中段的切片展示——完整内容 SHALL 始终存在于 DOM 中，使 `[[ui-search]]` 会话内全文搜索在 hydrate 后的匹配总数与全文一致（该契约见 `ui-search` 的搜索激活时全量 hydrate 规则）。限高仅约束视觉高度，不移除中段内容。

无论处于哪一档，输出块 SHALL 提供指向完整原文的复制入口（见 `[[copy-to-clipboard]]`）。

该展示 SHALL 覆盖顶层对话流与嵌套执行轨迹两条渲染路径：subagent / workflow 卡片内执行轨迹中的文本输出块 SHALL 受同一自适应契约约束，SHALL NOT 因位于嵌套轨迹而豁免限高。

#### Scenario: 短文本输出完整内联

- **WHEN** 一个对话流文本输出块的行数与字节数均低于限高阈值
- **THEN** 该输出 SHALL 完整渲染
- **AND** SHALL NOT 出现竖向内部滚动条
- **AND** SHALL NOT 显示"预览"提示

#### Scenario: 长文本输出限高预览且完整内容留在 DOM

- **WHEN** 一个对话流文本输出块达到限高阈值
- **THEN** 完整内容 SHALL 在响应式限高的内部滚动区域内渲染
- **AND** SHALL 显示总行数与总字节数及"预览"状态
- **AND** 中段内容 SHALL 保留在 DOM 中，不被首尾切片替换

#### Scenario: 长文本输出的全文搜索命中不因限高丢失

- **WHEN** 一个 1500 行的长文本输出块处于限高预览，唯一关键词位于第 700 行
- **AND** 用户激活会话内搜索并输入该关键词
- **THEN** 搜索命中总数 SHALL 计入该关键词
- **AND** 当前匹配 SHALL 可被滚动定位到（内容在 DOM 中，仅视觉限高）

#### Scenario: 嵌套执行轨迹内的文本输出同受限高约束

- **WHEN** subagent 或 workflow 卡片的执行轨迹中含一个达到限高阈值的文本输出块
- **THEN** 该输出块 SHALL 进入限高预览
- **AND** SHALL NOT 因位于嵌套轨迹而以完全不限高的方式渲染

#### Scenario: 复制针对完整原文而非可见片段

- **WHEN** 用户在限高预览状态下触发复制
- **THEN** 写入剪贴板的内容 SHALL 为完整原文
- **AND** SHALL NOT 仅为当前视口可见的文本

#### Scenario: 限高不破坏滚动稳定

- **WHEN** 一个长文本输出块随视口滚动进入 / 离开视口
- **THEN** 用户当前阅读位置 SHALL NOT 出现可感知跳动
- **AND** conversation 容器 SHALL NOT 采用"离屏估算高度占位、进视口换真高度"的容器级机制

### Requirement: 输出内部滚动区域键盘可访问

对话流文本输出块的限高内部滚动区域 SHALL 在内容沿任一轴（竖向或横向）实际溢出时，作为可聚焦的滚动 viewport 进入键盘 Tab 序列并可用键盘滚动；内容未沿任何轴溢出时，SHALL NOT 为该滚动 viewport 引入额外的 Tab 停靠点。输出块头部区域内的复制等动作控件 SHALL 始终键盘可达，不受滚动 viewport 是否可聚焦影响——二者是相互独立的 Tab 停靠点。可聚焦的滚动 viewport SHALL 提供可访问名称并显示可见的键盘焦点指示，SHALL 保留浏览器默认的边界滚动链（到达内部边界后继续滚动 conversation），SHALL NOT 制造滚轮陷阱。

#### Scenario: 竖向溢出时滚动区域可键盘进入

- **WHEN** 一个输出块内容竖向溢出其限高区域
- **THEN** 该滚动 viewport SHALL 可通过键盘 Tab 进入
- **AND** SHALL 可用方向键 / Page Up / Page Down / Home / End 滚动
- **AND** SHALL 显示可见的键盘焦点指示

#### Scenario: 仅横向溢出时滚动区域也可键盘进入

- **WHEN** 一个输出块含超宽单行、只有横向溢出而无竖向溢出
- **THEN** 该滚动 viewport SHALL 仍可通过键盘 Tab 进入并横向滚动

#### Scenario: 未溢出时不增加滚动 viewport 的 Tab 停靠点

- **WHEN** 一个输出块内容沿任一轴均未溢出（完整内联档）
- **THEN** SHALL NOT 为其滚动 viewport 引入额外的键盘 Tab 停靠点
- **AND** 头部区域内的复制等动作控件 SHALL 仍键盘可达

#### Scenario: 保留边界滚动链

- **WHEN** 用户在内部滚动区域滚动到其顶部或底部后继续同方向滚动
- **THEN** conversation 容器 SHALL 继续滚动
- **AND** 内部滚动区域 SHALL NOT 阻断滚动链

### Requirement: 对话流输出自适应展示的规模阈值

输出自适应展示的规模阈值 SHALL 是可被用户感知且可被测试断言的行为契约，并作为对话流文本输出（本 spec）与工具查看器输出（见 `[[tool-viewer-routing]]`）共用的判定基准。

- **规模度量**：字节数 SHALL 按内容的 UTF-8 字节长度计（与裁剪层记录的 `outputBytes` 同度量），SHALL NOT 用 UTF-16 码元数或 JavaScript 字符串长度；行数 SHALL 按换行符计数，末尾单个换行符 SHALL NOT 额外计为一空行。
- **升档判定**：字节数或行数任一达到或超过某档边界，即 SHALL 升入该档（`>=` 语义，非"约"）；任一维度达标即升档，保证极长单行等场景不因行数偏低而漏判。
- **限高档边界**：行数达到或超过 80，或字节数达到或超过 16 KiB（16384 字节）。
- **超大档边界**（仅适用于允许首尾切片的工具查看器输出，见 `[[tool-viewer-routing]]`；markdown 文本输出无此档）：行数达到或超过 1000，或字节数达到或超过 256 KiB（262144 字节）。

具体内部滚动区域的响应式像素高度为实现 tuning，不属本契约。

#### Scenario: 恰好达到限高档边界即升档

- **WHEN** 一个输出块行数恰为 80 或字节数恰为 16384
- **THEN** SHALL 进入限高预览档
- **AND** SHALL NOT 因"接近但未超过"而留在完整内联档

#### Scenario: 极长单行按字节数升档

- **WHEN** 一个输出块行数很少但 UTF-8 字节数达到或超过限高档边界
- **THEN** SHALL 按字节数进入限高预览或更高档
- **AND** SHALL NOT 因行数低而被判为完整内联档

#### Scenario: 多字节内容字节度量一致

- **WHEN** 一个输出块含大量多字节（如中文）字符
- **THEN** 分档使用的字节数 SHALL 为 UTF-8 字节长度
- **AND** SHALL 与裁剪层记录的 `outputBytes` 得出同一档位，不因前端改用字符串长度而错档

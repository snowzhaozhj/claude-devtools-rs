## ADDED Requirements

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

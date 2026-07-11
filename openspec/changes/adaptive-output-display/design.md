## Context

会话详情页（`SessionDetail`）的 conversation 容器是正常浏览时唯一的主滚动区。当前输出展示按内容规模不统一：

- 工具查看器（Bash/Default 走 `OutputBlock`，Read/Write/Diff 各自）已有**固定像素限高 + 内部滚动**（384 / 400 / 500px），但无信息气味：用户看不到总行数 / 字节数，也分不清"完整还是预览"；内部滚动 `<pre>` 不可聚焦，键盘用户进不去。
- AI 文本路径（display item 的 `output` / `user_message`、AIChunk 末尾 `lastOutput`）是 markdown prose，**完全不限高**——一条超长输出把 conversation 顶出很远，这是用户提出的核心痛点。
- 复制已是全文（`CopyButton` 拿完整 `code` / `output`），但只在 hover 出现（overlay 模式），键盘 / 触屏发现性差。

现有可复用基础设施：`get_tool_output` 懒加载完整 output；`outputBytes` / `outputOmitted` 让前端懒加载前即可估算规模；`ByteCappedCache`（`OutputBlock` 高亮缓存已用）；lazy markdown / 轻量高亮（`tool-viewer-routing::大文本工具详情交互优先渲染`）已是大文本的既有降级机制；`scrollAnchor` 主滚动锚定；`CopyButton` inline / overlay 两模式。

约束真相源：`session-display::按 Chunk 类型渲染对话流 / 长会话滚动高度保持稳定` 禁止在稳定容器上用"离屏估算高度占位、进视口换真高度"机制；`ui/CLAUDE.md` 硬约束竖向滚动容器配 `scrollbar-gutter: stable`（guard test 拦截）；`perf.md` 硬约束 IPC payload > 1MB 须瘦身、避免 hot-loop JSON parse；`DESIGN.md` Named Rules 定视觉边界。

**范围裁定**：本 change 经调研 + 用户拍板**只做前端展示层**。工具输出纳入会话内 Cmd+F 搜索、超大 output 应用内完整中段查看 + 后端分段加载、前端 output cache byte cap 化——这三项需新增后端 `search_tool_outputs` IPC + range/segment 通道 + 跨 5 capability 契约，属独立可交付工作，本 change 显式排除并开 GitHub issue 排期（见 `## 显式排除与 followup`）。

## Goals / Non-Goals

**Goals:**

- 为 output / tool output 建立**按内容规模自适应**的统一展示：短完整内联、中长响应式限高预览 + 信息气味、超大 top/tail 切片 + 省略接缝。
- 把当前完全不限高的 AI prose 输出路径纳入同一框架，消除"超长 prose 淹没对话流"。
- 截断 / 省略状态显式可见（信息气味 + 接缝，非渐隐）；复制始终针对完整原文且入口常驻可发现。
- 内部滚动区域实际溢出时键盘可进入、可滚动；响应式高度 + 稳定滚动槽；保留边界滚动链。
- 自适应展示遵守主滚动稳定契约；输出展开 / 懒加载不破坏贴底与锚定。
- 覆盖阈值边界 + loading / empty / error / disabled / hover / focus 全状态的前端测试 + 浏览器视觉验收。

**Non-Goals:**

- 不新增 IPC command、不改后端裁剪策略（`OMIT_TOOL_OUTPUT` 等）与 `get_tool_output` 协议。
- 不做工具输出的会话内 Cmd+F 搜索覆盖（deferred → issue）。
- 不做超大 output 应用内完整中段查看 / 后端分段加载（deferred → issue）。
- 不做前端 output cache byte cap 化（deferred → issue）。
- 不引入新第三方依赖 / 新顶层导航入口 / 独立 tab / modal。

## Decisions

### D1：分级判定用"原始规模元数据"（行数 + 字节数），不用渲染后 DOM 高度

- **候选**：(a) 渲染后测 `scrollHeight` 决定是否限高；(b) 用行数 / 字节数等稳定元数据分级。
- **选择**：(b)。字节数懒加载前用 `outputBytes`、加载后用真实文本长度校正；行数按换行符计数。
- **理由**：(a) 与 `session-display::长会话滚动高度保持稳定` 直接冲突——"渲染后测高再截"是被禁的"离屏估算高度占位后替换"的近亲，会在窗口缩放 / 懒加载前后让分级边界反复横跳触发滚动跳动。用不随视口变化的原始规模判定，边界稳定、可测。
- **风险**：极长单行 / 二进制替代文本让"行数"不足以代表规模 → 判定同时看行数与字节数，任一超阈即升级；元数据未齐时不得渲染整份超大内容再测量补算，退化为按已知字节数保守分级。

### D2：三档阈值（可测契约 + 实现 tuning 分离）

- **短（完整内联）**：行数与字节数均低于中长阈值 → 完整渲染，无竖向滚动、无预览提示。
- **中长（限高预览）**：超过短档未达超大档 → 响应式限高 viewport + 信息气味 header（总行数 · 总字节数 · "预览"）。
- **超大（top/tail 切片 + 省略接缝）**：达到超大档 → 在原位置只渲染首尾切片 + 省略接缝（标省略行数 / 字节数）+ 复制全文出口，不把超大 DOM 一次性灌进对话流。
- **阈值取值**：短 / 中长边界约 80 行或 16 KiB；超大边界约 1000 行或 256 KiB（呼应 `perf.md` 单 IPC payload 1MB 红线量级）。**这些数字作为可被用户感知 + perf 测试断言的行为契约进 spec NFR**；具体响应式像素高度是实现 tuning，不在 spec 层固定。
- **理由**：三档覆盖"短输出零摩擦、日志级中长限高可扫、超大不淹没且不炸 DOM"。超大改 top/tail 切片比现状"展开即渲染全量 DOM"更省（perf 净改善）。
- **风险**：阈值需实测校准；archive 前用真实长输出 fixture 验证边界体感，必要时调整并同步 spec NFR 数字 + baseline。

### D3：按"是否参与 Cmd+F 全文搜索"分离两类路径——prose 不切片、工具输出才切片（codex 二审 C3/W7 修订）

codex design 二审发现原方案对所有输出统一三档（含 top/tail 切片）会让"超大 prose 切片"与 `ui-search` 的"匹配总数与全文一致"契约直接冲突：切片后中段不在 DOM，搜索命中丢失；把全文渲回 DOM 又违反"只渲染首尾"。据此把两类路径彻底分开：

- **markdown prose 输出**（AIChunk output / user_message display item、lastOutput、嵌套 ExecutionTrace 内 prose）——全文首屏即在 payload（text step 来源，`contentOmitted` 裁的是前端不读的 `responses[].content`），且参与会话内 Cmd+F 全文搜索：**只有 {完整内联, 限高预览} 两档，不做 top/tail 切片**。限高预览下完整内容留在 DOM（仅 CSS 视觉限高 + 内部滚动），搜索 hydrate 后命中总数与全文一致。DOM 节点数不额外封顶——但这不劣于现状（现状 prose 完全不限高、也是全量 DOM），且视觉高度受控是纯改善。
- **行导向工具输出**（OutputBlock 承载的 Bash/Default output、Read/Write 代码、Diff）——不参与 Cmd+F（工具输出搜索已 deferred → #599），才安全采用三档含 top/tail 切片。切片仅对行导向纯文本/代码/diff（按行切分语义安全），不对 markdown 富文本（避免 W7 的围栏/表格跨切片结构断裂）。
- 超大工具输出的完整原文仍按现有懒加载获取（与用户展开任意工具的现状一致，不新增 payload 负担）；top/tail 是前端对已获取字符串的首尾切片；**应用内查看完整中段**（drill-in 完整查看器 / 分段加载）需后端 range/segment 通道 → deferred → #599；本 change 超大工具内容完整获取路径为"复制全文到外部"。
- **风险**：超大工具内容在应用内只能看首尾 + 复制全文，中段需外部查看——v1 明确取舍，issue 补齐。

### D6：工具输出懒加载态的稳定分档状态机（codex 二审 C1/C2 修订）

工具输出经 `outputOmitted` 裁剪后按需懒加载。原方案未定义"规模信号缺失 / 预估档与实际档不一致"时的行为，会击穿滚动稳定契约。修订为确定的状态转换：

- **规模信号优先级**：已加载真实内容 > `outputBytes` > 未知；用当前可得最高优先级信号分档。
- **裁剪空值 ≠ 0 规模**：`outputOmitted=true` 的空占位不判入短档。
- **规模未知时 fetch-first**：内容未加载且 `outputBytes` 缺失（老后端 / 解析层未填）→ 展开先触发懒加载，加载期以**稳定加载占位**（占位高度 = 限高档 viewport 高度）渲染，不先判短档再跳变。
- **加载后校正不改外层几何**：真实内容到达后确定最终档，但内容始终在**同一稳定外层限高 viewport** 内填充，外层占位几何加载前后不变——视口上方的块加载完成时不改主滚动几何。这是与 `session-display::长会话滚动高度保持稳定` 兼容的关键（针对 tab 恢复展开态 + `outputCache` 重置后异步补拉的时序）。

### D7：首尾切片渲染上限与切分安全（codex 二审 C5 修订）

原方案"只渲染切片"未给可验证上限，可被实现成接近完整 DOM，且超大单行会字符截断 / 首尾重叠重复。修订硬约束：

- **每侧上限**：首 / 尾各有最大渲染行数 + 最大渲染字节数上限（实现 tuning，初始每侧约 400 行 / 128 KiB），任一先达即停。
- **重叠规避**：总行数 ≤ 首尾两侧上限之和时不切片，退回限高预览完整渲染。
- **字符安全**：切分点落在 Unicode 码点 + 行边界（不拆组合字符簇 / 不在多字节字符中间截断）。
- **省略量**：接缝标注 = 总量 − 首尾实渲量，不少算 / 不重复计重叠。

### D8：内容源矩阵（codex 二审 C4 修订）

工具查看器分档依据"该 viewer 实际渲染的主内容面"，非固定取 output：Read/Bash/Default 及走 Default 渲染错误的失败工具 → output（error 附 errorMessage 计入）；Write → 待写入文件内容（input）；Edit/diff → old/new 差异（input，output 常为 Missing 不依赖）。避免"Write 2MiB input 但 10B 回执误判短档""Edit 无 output 无法分档"。

### D9：限高 viewport 是 lazy markdown 的外层稳定容器（codex 二审 W6 修订）

prose 走 lazy markdown：先写按全文估算的 inline `min-height` 占位、hydrate 后清除。若限高 `max-block-size` 落同一节点，min-height（可达数万 px）> max，hydrate 清除后骤缩违反滚动稳定。修订：限高 viewport SHALL 是 lazy-md 节点的**外层稳定容器**——observer target 与占位 min-height 在内层，`max-block-size` + 内部滚动在外层 viewport；占位清除只改内层、不改外层几何。

### D4：复制全文常驻可发现

- 自适应框架 header / utility row 常驻低权重 ghost「复制全文」（`CopyButton` inline 模式），替代 `OutputBlock` 当前 hover-only overlay；短输出同样常驻。
- **理由**：键盘 / 触屏用户需稳定可达的全文动作；hover-only 是发现性反模式（`DESIGN.md::The Floating Is Affordance, Not Decoration Rule` 的对立面——复制不是浮层 affordance 而是常驻工具）。
- **风险**：所有短输出都显示复制按钮增加密度 → 低对比 ghost + 共享一条 header 控制视觉权重。

### D5：无障碍内部滚动——仅实际 overflow 时进入 Tab 序列

- 限高 viewport 实际可滚动时才 `tabindex="0"` + 可达名（含工具名 / 内容规模）；短输出不加 Tab stop；工具 disclosure 收起后其内部滚动区退出 Tab 序列。
- 竖向滚动区配 `scrollbar-gutter: stable`（`ui/CLAUDE.md` 硬约束 + guard test）；重新评估 `OutputBlock` 现有 gutter 豁免注释——生命周期内滚动状态会变化的预览不再豁免。
- focus-visible 落 viewport 本身用现有焦点环，不用整块背景色；不用 `overscroll-behavior: contain` 造滚轮陷阱，Page Up/Down、Home/End、方向键、Space 按平台原生语义。
- **理由**：`PRODUCT.md::Accessibility & Inclusion` + WCAG 2.1.1 键盘可达滚动区。
- **风险**：长输出多时 Tab 步数增加 → "仅实际 overflow 可聚焦"控制数量。

### D10：apply 阶段决策——主 AI 回复（lastOutput）不限高 + prose 用轻量框（用户拍板）

apply 中接入 prose 路径时向用户确认两点，结论反转 / 细化了原方案：

- **主 AI 回复不限高**：原 spec 把"AIChunk 末尾的最后输出"纳入自适应两档。实测判断：顶层 SessionDetail 的主 AI 回复是用户要阅读的正文，塞进 ~30rem 限高滚动框反而降低可读性。**反转**：顶层主回复 SHALL 始终完整内联，不进限高；只有 output / user_message 展示项（在可折叠 disclosure 内）与嵌套执行轨迹内被平铺为普通输出项的文本受两档约束。已同步 `session-display` spec delta（新增"主 AI 回复始终完整内联不限高"Scenario）+ proposal + tasks（3.2 改为"lastOutput 保持不限高"）。
- **prose 用轻量框**：prose 限高时若复用工具输出的 code-bg 边框 header，会把普通文本显示得像代码块。**细化**：`AdaptiveOutputFrame` 加 `variant='prose'`——透明 / surface 底 + 细下边框承载 scent + 复制，不套 code-bg 边框；`variant='code'`（默认）仍为工具输出的 code-bg 框。
- 依据：`PRODUCT.md::Design Principles`（审计优先——正文优先可读）；`DESIGN.md::The Tool Density Rule`（不为普通文本套代码块 chrome）。

### D-V1 ~ D-V4（视觉决策）

> 来自设计 teammate `/impeccable shape` 产出、按本 change 裁剪后的关键视觉决策（原 drill-in 完整查看器 D-V 因范围裁定 deferred to issue），与 D1–D5 共享同一审计 / 反转规则。

### D-V1：短 / 中长 / 超大三档留在 conversation，不新增 modal / 侧栏 / tab

- **候选**：(1) 无限展开；(2) 完整查看走 modal；(3) 三档全部在原 conversation 位置内解决——短内联、中长限高预览、超大 top/tail 切片 + 省略接缝 + 复制全文。
- **选择**：(3)。conversation 始终是唯一主滚动；中长预览只是局部可键盘进入的辅助滚动；超大不弹层、不跳走。
- **理由**：modal 叠第二主滚动 + backdrop glass（违 `DESIGN.md::The No Decorative Glass Rule`）；新 tab / 侧栏破坏上下文 / 挤压 conversation。
- **依据**：`PRODUCT.md::Design Principles`（审计优先 / 熟悉即效率 / 密度有层次）+ `PRODUCT.md::Anti-references`（拒重复卡片 / 无意义新浮层）；`DESIGN.md::The Border Before Shadow Rule`、`DESIGN.md::The No Decorative Glass Rule`。

### D-V2：统一内容规模分级替代各 viewer 独立固定像素限高

- **候选**：(1) 保留各 viewer 固定像素；(2) 只按工具类型定高；(3) 所有文本 / 代码 / diff viewer 共享"完整内联 / 限高预览 / top-tail 切片"三级语义，工具类型只决定渲染方式不决定信息完整性。
- **选择**：(3)。中长内容响应式 block-size（初始校准约 `clamp(12rem, 42dvh, 30rem)`，至少可读 10–12 行，最终沉淀为共享 token）；分级由原始行数 / 字节数决定，不以换行后 DOM 高度反推。
- **依据**：`PRODUCT.md::Design Principles`（密度有层次 / 实时但不闪烁）；`DESIGN.md::The Tool Density Rule`、`DESIGN.md::The Machine Information Rule`。

### D-V3：截断用显式信息气味 + 省略接缝，不用渐隐遮罩

- **候选**：(1) 底部渐变 fade 暗示；(2) 只显示"展开"图标不说范围；(3) 共享 header 显示总行数 / 总字节数 / "预览"状态；超大 top/tail 间插结构性省略接缝说明省略量 + 复制全文。
- **选择**：(3)。top/tail 是同一输出表面两个连续片段，不各自包装成卡；接缝用中性文字 + 细分隔线 + 明确文案，不用渐变 / 阴影 / 彩色装饰；用户能立即区分"完整 / 限高但完整 / 只首尾切片"。
- **依据**：`PRODUCT.md::Design Principles`（审计优先）；`DESIGN.md::The Status Owns the Color Rule`、`DESIGN.md::The Border Before Shadow Rule`、`DESIGN.md::The Machine Information Rule`。

### D-V4：限高预览是显式、稳定、可退出的辅助滚动区域

- **候选**：(1) 固定高度 + 按需滚动条 + 不进 Tab 序列；(2) 捕获滚轮阻止滚动链；(3) 仅实际 overflow 可聚焦，响应式高度 + 稳定滚动槽，保留边界滚动链。
- **选择**：(3)（同 D5）。实际可滚动才 `tabindex="0"` + 描述性可达名；`scrollbar-gutter: stable` 不再豁免；focus-visible 落 viewport；不用 `overscroll-behavior: contain`；键盘滚动按平台原生语义。
- **依据**：`PRODUCT.md::Accessibility & Inclusion`；`DESIGN.md::The Status Owns the Color Rule`、`DESIGN.md::The Tool Density Rule`。

## Visual Contract

### Surface Decision

自适应输出仍属 SessionDetail 的 execution trace 与对话流，不新增独立导航入口 / 常驻侧栏 / 嵌套卡片 / modal。正常浏览时：短输出完整内联；中长输出在原工具 disclosure 或 prose 位置形成有明确边界的辅助滚动 viewport；超大输出在原位置 top/tail 切片 + 省略接缝 + 复制全文。conversation 始终是唯一主滚动。该选择链回 `PRODUCT.md::Design Principles`（审计优先 / 熟悉即效率 / 密度有层次），主动避开 `PRODUCT.md::Anti-references` 的重复卡片、装饰浮层、为外观重造控件。

### Visual Layer

- 输出框架用现有代码 / diff surface + 细边框 + 单一共享 header，不在 disclosure 内再包 raised card。引用 `DESIGN.md::The Border Before Shadow Rule`、`DESIGN.md::The No Decorative Glass Rule`。
- 行数 / 字节数 / 范围 / 语言 / 省略量用 mono metadata；"预览""已省略"等自然语言用 UI 字体。引用 `DESIGN.md::The Machine Information Rule`。
- header / metadata / 省略接缝 / ghost actions 用暖中性色；蓝色只用于 focus-visible / 链接，红色只用于真实输出错误。引用 `DESIGN.md::The Status Owns the Color Rule`、`DESIGN.md::The Warm Neutral Rule`。
- 三档不靠字号放大或彩色 badge 区分，靠内容完整性 / 边界 / metadata 区分。引用 `DESIGN.md::The Tool Density Rule`。
- 中长预览用响应式 block-size 而非各 viewer 独立固定像素；竖向 overflow 用稳定 gutter；横向长行仍在代码 viewport 内滚动，不把 SessionDetail 撑出 pane。
- 超大 top/tail 共享同一背景与外边界；中间只放一条省略接缝，不做两张卡、不加渐变 fade；接缝文案至少含省略量，与"复制全文"动作相邻。
- 所有动作用原生 button 语义；可滚动预览仅确实 overflow 时进入 Tab 序列；按钮与滚动 viewport 都有清晰 focus-visible，不靠 hover 暴露关键动作。

### State Coverage

| 状态 | 视觉与交互 | 实现位置 |
|---|---|---|
| **loading** | 保留已有 preview 与几何高度，utility row 显示静态"正在载入…" + `aria-busy`；禁用依赖全文的复制全文。不清空旧内容、不插 shimmer、不让 block 高度反复变化。 | 自适应输出框架 header / utility row；懒加载状态由 SessionDetail 输出控制器提供。 |
| **empty** | 紧凑中性空输出行 + `0 行 · 0 B`；无竖向滚动、无省略接缝；空内容复制结果明确为空，不伪装成成功。 | 输出框架 body + metadata。 |
| **error** | 工具执行失败沿用输出错误语义；完整原文无法获取（懒加载失败 / Missing）时复制入口 SHALL 禁用（保留原因标签），SHALL NOT 降级为复制可见切片。**不引入新的复制失败反馈**——复制成功 / 失败反馈沿用既有 `copy-to-clipboard::点击 CopyButton 复制文本并显示反馈`（成功切换图标 / 失败静默降级），避免与既有契约冲突（codex 二审 W9）。 | 输出 body（执行错误）/ utility row（懒加载失败禁用复制）。 |
| **disabled** | disabled action 保留可读标签 / tooltip + 原生 disabled / `aria-disabled`，不响应 Enter/Space；原因覆盖"正在加载""完整原文不可用"。 | 共享 header actions。 |
| **hover** | 只提高对应 ghost control 背景 / 文字对比；不抬升整张输出、不加阴影、不改尺寸；关键动作不只在 hover 存在。 | 输出 header / utility row。 |
| **focus-visible** | button 用现有焦点环；可滚动 preview 在 viewport 边界显示清晰 outline + 含工具名与规模的可达名；鼠标点击不强制显示键盘焦点样式。 | 所有 header controls + 实际 overflow 的 preview viewport。 |
| **complete inline** | metadata 表明完整规模；正文自然撑开，无竖向 scrollbar，无"预览 / 省略"提示。 | 自适应输出框架 body。 |
| **bounded preview** | header 显示总行数 / 总字节数 / "预览"；正文限高 + 稳定 gutter；复制全文指向原文。 | 输出框架 header / preview viewport。 |
| **top / tail summary（仅行导向工具输出）** | top 与 tail 连续显示；中间接缝说明省略量并旁置复制全文；不用 fade 暗示、不让用户误以为两段相邻。markdown prose 不进本态（改限高预览完整渲染）。 | 超大行导向工具输出切片 renderer。 |
| **copied** | 复制成功用现有轻量反馈，不改正文布局；按钮标签 / tooltip 明确"复制全文"；复制失败有文字反馈。 | 共享复制 action。 |
| **narrow pane** | metadata 换行或收敛为紧凑组合；文件名截断保留完整 tooltip；复制全文保持可见不被挤出 pane。 | 输出 header 响应式布局。 |

### DESIGN.md delta plan

archive 前运行 `/impeccable extract`，只把跨 OutputBlock / Read / Write / Diff / 默认 viewer 与 prose 路径重复成立的内容沉淀进 `DESIGN.md`：

1. **共享组件契约**：Adaptive Output Frame（统一 header / 规模 metadata / 三级分级 / 常驻复制 / 可滚动 viewport）；Output Omission Seam（top/tail 省略量提示 + 复制全文）。
2. **候选 token**：输出预览响应式最小 / 最大 block-size；省略接缝间距与分隔边界（颜色优先 alias 现有 neutral text / border，不新增色相）。
3. **候选 Named Rules**：`The Conversation Owns the Scroll Rule`（conversation 是主滚动；短内联、中长仅 overflow 时形成可聚焦辅助滚动、超大 top/tail 切片，禁无限展开淹没 conversation）；`The Preview Must Declare Itself Rule`（任何非完整可见输出必须声明总规模 / 预览省略状态；复制不静默降级到可见片段）。
4. **现有章节更新**：`DESIGN.md::Code, diff, and output` 把各 viewer 统一 header / action 语言扩展为自适应分级；记录动态竖向输出 viewport 用稳定 scrollbar gutter，仅生命周期内滚动状态不变的 surface 才允许豁免。
5. **提取门槛**：archive 前以 OutputBlock / Read-Write / Diff 至少三类 viewer 的实装 + 浅 / 深 / 窄 pane 视觉验收为证据；未形成复用的局部样式留组件实现。

## Risks / Trade-offs

- **超大内容应用内只能看首尾**：完整中段查看 deferred → 复制全文到外部。缓解：top/tail + 省略量 + 复制全文让用户仍能获取全部内容；完整查看器随 issue 补齐。
- **阈值误判体感**：静态阈值在超长单行 / 巨型 diff 下可能突兀 → archive 前真实长输出 fixture 视觉验收校准，必要时同步 spec NFR 数字。
- **prose 路径接入自适应框架的渲染耦合**：output / user_message / lastOutput 走 lazy markdown，限高需与 lazy 渲染 + 搜索 hydrate（`ui-search` 搜索前 flushAll）兼容 → 限高用 CSS block-size 不改 DOM 结构，不干扰 lazy hydrate；加 Playwright 覆盖"搜索 hydrate 后限高仍生效"。
- **scrollbar-gutter guard 回归**：移除 `OutputBlock` 现有豁免可能触发 guard test → 同 commit 更新 guard 期望 + 加 `stable`。
- **Codex 设计二审不可用**：本地 Codex CLI 未安装（`Codex skipped: CLI not installed`）——design 阶段异构二审缺席，改由 spec-fidelity / 视觉 teammate + apply 阶段 codex（若届时可用）+ 人审补偿。

## Migration Plan

- 纯前端 UI 行为演进，无数据迁移、无 IPC 协议变更、无 breaking API。
- 分阶段：先建自适应框架 + 三档分级（含 OutputBlock 与 AI prose 路径）→ 再接工具查看器（Read/Write/Bash/Default/Diff）→ 最后 a11y 打磨 + 视觉验收 + DESIGN.md extract。
- 回滚：改动集中在 SessionDetail 相关组件与样式，可按 commit 回退；不触后端，回退不影响数据正确性。

## Open Questions

- 三档阈值最终数值需实测校准（起点 80 行 / 16 KiB 与 1000 行 / 256 KiB）——apply 阶段 fixture 验收拍板并回填 spec NFR。

## 显式排除与 followup（GitHub issue）

以下经调研需后端 `search_tool_outputs` typed IPC + range/segment 通道 + 跨 `ipc-data-api` / `http-data-api` / `session-search` / `ui-search` 契约，属独立可交付工作，本 change 不含，archive 前开 GitHub issue 排期，并在相关 spec 显式标注边界：

1. **会话内 Cmd+F 覆盖工具输出**：当前对话级搜索跳过 `<pre>` / `<code>` 与 `outputOmitted` 内容；工具输出全文搜索 + 命中定位（含折叠 / 省略中段 / 嵌套 subagent trace）需 `search_tool_outputs`（复用 parsed-message cache + byte-capped LRU + 稳定 occurrence locator）。
2. **超大 output 应用内完整中段查看 + 分段加载**：需 `get_tool_output_segment(offset, before, after)` 或 range/asset 通道，避免单条 >1MB output 整体 IPC 与超大 DOM。
3. **前端 output cache byte cap 化**：`SessionDetail` outputCache（count-only 200）与 `ExecutionTrace` 无界 cache 改 `ByteCappedCache` 双闸门，key 含 fingerprint。
4. **HTTP vs Tauri detail omission 对齐**：普通 HTTP `GET /api/sessions/{id}` 当前不 `apply_omissions`，与 Tauri 首屏裁剪分叉，影响搜索 / 懒加载 E2E 一致性——随搜索工作一并对齐。

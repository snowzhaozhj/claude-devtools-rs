## Why

会话详情中的输出展示缺少"内容规模"这一维度的统一处理：AI 文本输出路径（Output / User message display item、AIChunk 末尾 lastOutput）完全不限高，一条超长输出会把主对话顶出很远；工具查看器虽有固定像素限高 + 内部滚动，却没有信息气味（看不到总行数 / 字节数、分不清"预览还是完整"），内部滚动区域键盘不可达，复制全文只在 hover 出现。需要为 output / tool output 建立统一、可访问、带截断提示的自适应展示契约。

## What Changes

- 按内容规模自适应展示 output / tool output：短内容完整内联；中长内容进入响应式限高预览 + 信息气味 header（总行数 · 总字节数 · "预览"状态）；超大内容在原位置渲染首尾（top/tail）切片 + 省略接缝（标注省略行数 / 字节数），不一次性把超大 DOM 灌进对话流。
- 把当前**完全不限高**的 AI 文本输出路径（Output / User message / lastOutput prose）纳入同一自适应框架，消除"超长 prose 淹没对话流"。
- 截断 / 省略状态 SHALL 显式可见（信息气味 + 省略接缝），不使用渐隐遮罩暗示；复制 SHALL 始终针对完整原文，且复制入口常驻可发现（不再仅 hover 出现）。
- 内部滚动区域 SHALL 在实际溢出时键盘可进入、可滚动（`tabindex` + focus-visible + 稳定滚动槽），保留浏览器默认边界滚动链。
- 自适应展示 SHALL 遵守既有主滚动稳定契约（不引入"离屏估算高度占位后替换"机制），输出展开 / 懒加载不破坏贴底与滚动锚定。
- 增加覆盖阈值边界、空 / 加载 / 错误 / 截断 / hover / focus 全状态的前端测试与真实长输出 fixture 的浏览器视觉验收。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-display`: 为对话流中的 output / User message / lastOutput 输出路径引入自适应长内容分级、信息气味、可访问内部滚动，并重申与主滚动稳定的兼容。
- `tool-viewer-routing`: 为工具查看器（Read / Write / Bash / Default / Diff）引入统一的自适应限高预览、信息气味、超大内容 top/tail 切片 + 省略接缝、键盘可访问滚动，替代当前各自固定像素限高的隐式行为。
- `copy-to-clipboard`: 输出表面的"复制全文"入口 SHALL 常驻可发现（不再仅 hover 出现），并明确复制内容为完整原文而非可见预览。

## Impact

- 前端：`SessionDetail.svelte`（output / user_message / lastOutput prose 路径 + 自适应框架接入）、`OutputBlock.svelte`、Read/Write/Diff 等工具查看器、`CopyButton` 使用方式，及相关样式与 `scrollbar-gutter` guard。
- 数据流：复用现有 `outputBytes` / `outputOmitted` / `getToolOutput` 懒加载链路估算与获取内容；**不新增 IPC command、不改后端裁剪策略**。
- 测试：Vitest 覆盖分级判定 / 全文复制 / 信息气味文案；Playwright 覆盖长输出交互、键盘滚动、截断态与滚动稳定性；真实长输出 fixture 做浏览器视觉验收。
- 性能：不劣于现状——工具输出仍按现有懒加载在展开时获取；超大内容改为 top/tail 切片渲染，比当前"展开即渲染全量 DOM"更省。无新第三方依赖、无 breaking API。

## 显式排除（本 change 不做，开 GitHub issue 单独排期）

以下经调研需新增后端 `search_tool_outputs` typed IPC + 超大 output 分段加载 + 前端 output cache byte cap 修复，跨 `ipc-data-api` / `http-data-api` / `session-search` / `ui-search` 多 capability，属独立可交付工作，**本 change 不含**，spec 中显式声明边界后开 issue 排期：

- **会话内 Cmd+F 覆盖工具输出**：当前对话级搜索仍跳过 `<pre>` / `<code>` 与 `outputOmitted` 内容——本 change 不改变该边界；工具输出纳入 Cmd+F 全文搜索留 issue。
- **超大 output 完整中段查看 + 分段加载**：本 change 对超大内容提供 top/tail 切片 + 复制全文出口；在应用内查看完整中段（需后端 range/segment 通道避免 >1MB payload 与超大 DOM）留 issue。
- **前端 output cache byte cap**：`SessionDetail` outputCache（count-only 200）与 `ExecutionTrace` 无界 cache 的 byte cap 化，与上述搜索 / 分段工作耦合，一并留 issue。

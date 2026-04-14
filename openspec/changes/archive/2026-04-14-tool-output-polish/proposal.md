## Why

Tool output（代码块）的视觉质量是日常使用体感最直接的部分。当前存在三个问题：代码块背景色与外层容器融为一体导致边界不清；长 JSON / 长文本 output 占据过多纵向空间缺乏折叠策略；`white-space: pre-wrap` 强制断行导致宽内容（如长路径、日志行）可读性差。

## What Changes

- **代码块视觉边界强化**：增大代码块与外层容器的背景色对比度，加深 `--code-bg` 或给 `.code-block` 增加更明显的边框/阴影
- **长 output 折叠策略**：统一 Tool Viewer 的 output 区域，默认折叠超过阈值（如 15 行）的内容，点击展开查看全部
- **水平滚动替代强制断行**：代码块使用 `white-space: pre; overflow-x: auto` 替代 `pre-wrap`，保持原始格式，长行通过水平滚动查看
- **统一各 Viewer 的 output 样式**：抽取通用 output 容器样式，消除 Default/Bash/Read 等 Viewer 之间的样式重复

## Capabilities

### New Capabilities

（无——纯 UI 样式改动，不引入新数据层 capability）

### Modified Capabilities

（无——不涉及 spec 级别的行为变更）

## Impact

- `ui/src/app.css`：调整 CSS 变量（代码块背景色、边框）
- `ui/src/components/tool-viewers/*.svelte`：所有 5 个 Tool Viewer 组件的 output 区域样式和折叠逻辑
- `ui/src/routes/SessionDetail.svelte`：prose 内的 `<pre>` 代码块样式
- 无后端改动、无 API 变更、无依赖变化

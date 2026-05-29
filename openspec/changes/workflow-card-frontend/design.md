# Design: WorkflowCard 6-state rendering

## Decisions

### D1: WorkflowItem 作为 AIChunk 的可选字段

`AIChunk.workflows?: WorkflowItem[]` — 后端 `skip_serializing_if = Vec::is_empty`，前端 `?? []` 兼容老后端。

### D2: DisplayItem 扩展而非独立渲染路径

新增 `WorkflowDisplayItem` 到 `DisplayItem` union，复用 `buildDisplayItems` 的时序池排序机制，与 tool/subagent/teammate 统一穿插。

### D3: WorkflowCard 独立组件（不复用 SubagentCard）

虽然视觉 token 复用 SubagentCard 家族（gap/padding/radius/font-mono），但语义完全不同（多 phase + fan-out agents vs 单 subagent trace），独立组件避免条件分支膨胀。

### D-V1: 复用 SubagentCard header token 家族

字号、间距、mono 字体数字、chevron 旋转、card-bg/border 全部复用已有 CSS 变量。

### D-V2: 折叠态单行 8 元素 flex

`chevron · icon · name · "N phases · M agents" · status · tokens · duration`

### D-V3: Phase 纵向分组 + fan-out chip 横排

2px `border-left` thread rail；chip 内 8px status dot + label + meta。

### D-V4: One Live Signal Rule 遵从

仅 header 一个 spinner（running 态）。chip dot 全静态——running agent 用静态蓝色圆点，不带旋转。

### D-V5: Script 默认折叠

"View script" disclosure chevron 默认收起，点击展开 `<pre>` 块。

### D-V7: 启动失败不画空卡

Launch error 通过 tool_result.is_error 走正常 BaseItem 错误渲染路径，不产出 WorkflowDisplayItem。

### D-V8: 诚实约束

Running 态禁画假进度条/百分比。仅 spinner + "details available after completion"。

## Agent chip status dot 颜色映射

| status | color |
|--------|-------|
| done | `--color-success-bright` (green) |
| failed | `--color-error` (red) |
| running | `--color-accent-blue` (static, no animation) |
| queued | `--card-icon-muted` border, transparent fill |
| cached | same as queued |

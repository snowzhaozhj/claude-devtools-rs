# Proposal: Per-turn Context Badge + Visible Context

## Summary

为每个 AI turn 的 header 行增加 "Context +N" 可点击 badge，展示本轮新注入上下文的 category 明细与 token 数；同时在现有 token 统计 popover 中新增 "Visible Context" 折叠段，显示全 session 累积 context 按类别的占比。

## Motivation

用户需要两个层面的 context 消耗透明度：

1. **增量视角**（per-turn badge）："这一轮新增了什么到 context window？"——帮助理解单轮 token 消耗的构成
2. **累积视角**（visible context）："整个 session 的 context window 被什么占着？"——帮助做优化决策（如精简 CLAUDE.md rules）

TS 原版已有此功能，Rust 端口后端数据层已就绪（`cdt-analyze::context` 的 `stats_map` 按 AI group 聚合），但 IPC 未暴露 per-turn 数据，UI 也缺少对应组件。

## Scope

### In scope

- 后端：`SessionDetail` IPC 新增 `per_turn_context` 精简 summary map
- 后端：暴露 `stats_map` 中已有的 per-group 计算结果（非新增计算）
- 前端：新建 `ContextBadge.svelte` 组件（click → popover）
- 前端：token popover 改为 click 触发 + 新增 "Visible Context" 折叠段
- 前端：两 popover 互斥逻辑
- A11y：ARIA role/expanded/label + focus trap + keyboard dismiss
- 测试：IPC contract test + vitest 组件测试

### Out of scope

- 百分比可视化 stacked bar（P3，后续 PR）
- badge → ContextPanel 联动高亮（P2，后续 PR）
- ContextPanel 本身的改动

## Capabilities

- `context-tracking`（主 capability，spec delta）
- `ipc-data-api`（IPC 字段新增）

## Design decisions deferred to design.md

- `per_turn_context` 的精确字段结构
- Badge 空态阈值（哪些 count 不显示）
- Token popover click 改造的交互细节
- Visible Context 折叠段的信息层级

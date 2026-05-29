# Tasks: WorkflowCard 6-state rendering

## 1. 数据层
- [x] 1.1 api.ts 加 WorkflowPhase / WorkflowAgent / WorkflowItem 类型
- [x] 1.2 AIChunk 加 workflows? 可选字段
- [x] 1.3 displayItemBuilder 加 WorkflowDisplayItem + pool 处理 + summary 计数 + chunkDigest
- [x] 1.4 toolHelpers getToolSummary 加 "Workflow" case

## 2. 组件
- [x] 2.1 WorkflowCard.svelte 6 态完整实现
- [x] 2.2 ExecutionTrace.svelte 加 workflow 渲染分支
- [x] 2.3 SessionDetail.svelte 加 workflow 渲染分支

## 3. Fixture
- [x] 3.1 workflow-rich.ts 4 变体 mock（completed / partial_failure / running / empty）+ launch error
- [x] 3.2 index.ts 注册

## 4. 测试
- [x] 4.1 vitest displayItemBuilder workflow 单测
- [x] 4.2 vitest toolHelpers Workflow 单测
- [ ] 4.3 Playwright e2e 截图（deferred — 需真浏览器渲染环境）

## 5. Spec delta
- [x] 5.1 session-display spec 加 WorkflowCard 渲染 Scenario
- [x] 5.2 tool-viewer-routing spec 加 Workflow routing case

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

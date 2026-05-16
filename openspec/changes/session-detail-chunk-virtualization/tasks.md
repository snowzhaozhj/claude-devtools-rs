## 1. Dynamic virtualizer 基础设施

- [x] 1.1 新增动态高度 virtualizer 模块，支持 count、估算高度、实测高度、overscan、visible range、top/bottom spacer、scrollToEnd 与 reset
- [x] 1.2 为 virtualizer 补 vitest，覆盖 range 计算、实测高度覆盖估算、滚动到底、count/key 变化 reset

## 2. SessionDetail 接入

- [x] 2.1 在 SessionDetail 主 conversation 接入 chunk 级 virtual rows、spacer、ResizeObserver 测量与回滚常量
- [x] 2.2 保持 lazy markdown / Mermaid / image lazy load 的 root 与 flushAll 行为不回退
- [x] 2.3 保持工具展开/收起、lazy tool output、SubagentCard 与 teammate message 渲染行为不回退
- [x] 2.4 保持 file-change 自动刷新贴底、非贴底不抢 scroll、per-tab scroll 保存/恢复与 openOrReplaceTab 状态隔离
- [x] 2.5 搜索激活时保证全文可搜索，必要时临时全量渲染并继续使用现有 DOM highlight/navigation

## 3. UI 测试与验证

- [x] 3.1 增加/更新 SessionDetail 相关 vitest，覆盖长 session 仅渲染窗口、搜索远端 chunk、展开后高度更新、openOrReplaceTab 不复用旧测量
- [x] 3.2 增加/更新 Playwright user story 或现有 e2e，覆盖 mock fixture 长会话滚动、搜索、贴底刷新关键路径
- [x] 3.3 运行 `npm run check --prefix ui` 与相关 vitest/e2e
- [x] 3.4 用浏览器 mock fixture 做滚动 smoke，确认长会话滚动、搜索、工具展开无明显回归
- [x] 3.5 运行 `openspec validate session-detail-chunk-virtualization --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

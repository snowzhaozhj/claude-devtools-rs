## 1. 前端渲染隔离

- [x] 1.1 定位 SessionDetail chunk / message 外层 DOM 结构，选择不影响搜索、lazy markdown、Mermaid 的稳定块边界
- [x] 1.2 为 chunk / message 外层添加 `content-visibility: auto`、`contain-intrinsic-size` 与低风险 `contain` 样式
- [x] 1.3 验证样式不改变 chunk DOM 顺序、展开状态、贴底滚动和搜索 flush 行为

## 2. 代码高亮策略

- [x] 2.1 调整 `ui/src/lib/render.ts`，声明语言且受支持时继续按语言高亮
- [x] 2.2 未声明语言或超过阈值的 fenced code block 按 plaintext 渲染，不走不受限 `highlightAuto`
- [x] 2.3 补充 unit test 覆盖声明语言、未声明语言、大块未声明语言三类代码块

## 3. UI 验证

- [x] 3.1 用 mock UI 或 Playwright/浏览器验证 SessionDetail 长会话滚动无明显功能回归
- [x] 3.2 验证搜索可命中离屏内容，lazy markdown 仍按进入视口或 `flushAll()` 渲染
- [x] 3.3 验证 Mermaid 进入视口后仍渲染图表并可切换 Code/Diagram
- [x] 3.4 运行 `npm run check --prefix ui` 与相关 unit/e2e 测试
- [x] 3.5 运行 `openspec validate session-detail-scroll-cpu-opt --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

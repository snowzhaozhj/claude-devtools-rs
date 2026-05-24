## 1. ui — CSS 与模板清理

- [x] 1.1 删除 `ui/src/routes/SessionDetail.svelte` 中 `.msg-row-contained` 样式定义整段（含 `content-visibility: auto` / `contain: layout style` / `contain-intrinsic-size: auto 220px` 三个属性，以及 `:global(.msg-row-contained:has(.mermaid-block)) { content-visibility: visible; contain: none; }` 豁免规则）
- [x] 1.2 删除 `SessionDetail.svelte` 模板中 4 处 class 应用：UserChunk 容器（`msg-row-user msg-row-contained`）/ AIChunk `.ai-body` 的 `class:msg-row-contained={...}` 表达式（含上方解释 ongoing 例外的注释段）/ SystemChunk 容器（`msg-row-system-left msg-row-contained`）/ CompactChunk 容器（`msg-row-compact msg-row-contained`）
- [x] 1.3 删除 `SessionDetail.svelte` 中三处直接引用 `.msg-row-contained` / `content-visibility:auto` trade-off 的遗留注释（位于平滑滚动 / scrollend 兜底 / 模板段三处）
- [x] 1.4 保留 `ui/src/lib/lazyMarkdown.svelte.ts` 现有契约不动；保留 `.lazy-md[data-rendered="1"] { min-height: 0 !important }` 全局规则不动（与 lazy markdown 渲染配套）

## 2. ui — 测试调整

- [x] 2.1 删除 `ui/src/components/SessionDetail.test.svelte.ts` 中三条与回退机制直接绑定的 test case 整体：(a) "IPC 返回的 chunks 渲染 containment 边界且不包住 AI header"（验证 D2b 的 AI header 豁免契约，删类后无契约可验）；(b) "恢复展开状态时工具列表容器不使用 containment"（断言 `.ai-tools-section` 不挂 contain class，删类后无意义）；(c) "含 mermaid 的 contained 区域通过 CSS 关闭 content-visibility"（验证 mermaid 豁免规则，删类后无规则可验）
- [x] 2.2 跑 `pnpm --dir ui run check` 验证类型 / svelte-check 干净
- [x] 2.3 跑 `pnpm --dir ui run test:unit` 验证 vitest 全绿，特别确认 SessionDetail.test 剩余 case 不受影响

## 3. 防回归测试

- [x] 3.1 在 `SessionDetail.test.svelte.ts` 新增一条 test case：渲染含多个 UserChunk / AIChunk / SystemChunk / CompactChunk 的 fixture 后，断言 `.msg-row-contained` 类完全不存在 DOM 中、且无任何元素 computed `content-visibility` 为 `auto`（防止未来 PR 又引入同类机制）
- [x] 3.2 反转 fix 验证 3.1：临时把 `.msg-row-contained { content-visibility: auto }` 复原一行加回 SessionDetail.svelte，跑测试应 fail；恢复 fix 后应 pass；commit 前确认 cycle 完成

## 4. 本地性能验证（merge gate）

- [ ] 4.1 在长会话（至少 100 chunk 或本地最长可用 session）上手动 `just dev` → Activity Monitor 抓 5 秒滚动样本，记录 `claude-devtools-tauri` 进程 CPU%
- [ ] 4.2 若 CPU 持续 < 15% → 通过本 gate；若 ≥ 15% → 暂缓 merge，开 followup issue 评估 ResizeObserver 测量缓存方案（与本 PR 解耦）
- [ ] 4.3 同会话上跑 console jitter monitor 脚本（`scrollHeight` 变化 10 秒采样），断言变化次数 ≤ 2 / 总幅度 ≤ 50 px

## 5. 文档与索引同步

- [x] 5.1 检查 `openspec/specs/session-display/spec.md` 主 spec 同步是否会被 archive 自动处理（archive 阶段 `openspec archive` 会自动 sync delta，本步无需手工改主 spec）
- [x] 5.2 跑 `openspec validate revert-scroll-jitter-content-visibility --strict` + `bash scripts/check-spec-purity.sh` 双重 spec 合规
- [x] 5.3 在 `.claude/rules/perf.md` 的"反模式清单"段加一条交叉引用 spec：禁止在 SessionDetail 对话流容器 / chunk 容器 / message 级稳定块容器上用 `content-visibility: auto` + `contain-intrinsic-size` 这类"估算占位高度替代真实高度"机制做滚动性能优化，引用 `session-display::按 Chunk 类型渲染对话流::长会话滚动高度保持稳定` Scenario 作为约束源

## 6. 提交前自检

- [x] 6.1 跑 `just preflight` 全套 fmt / lint / test / spec-validate 一把梭
- [x] 6.2 复查 git diff 确认本次改动仅触及：`SessionDetail.svelte` + `SessionDetail.test.svelte.ts` + `openspec/changes/<slug>/**`；无误改其它文件

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（PR 阶段二审 11 findings，9 仅记录、2 建议但不阻塞；F1 扩大防回归 test 覆盖到 `.ai-body` / `.ai-tools-section` 已采纳；F9 perf.md archive 后引用自动生效跳过）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

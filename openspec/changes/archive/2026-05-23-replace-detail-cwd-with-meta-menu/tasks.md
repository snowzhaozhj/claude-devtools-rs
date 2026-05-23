## 1. 设计预研与决策落定

- [x] 1.1 核实 `lib/components/Dropdown.svelte` 现有 API：是否支持 action menu 模式（item click → side effect 而非 value-select）。（已知初判：仅 value-select，`{ value, options, onChange }` 签名；本 change 不扩 Dropdown，直接在 `SessionMetaMenu.svelte` 内部实现 action menu。决策记录追加进 design.md `## Decisions` 段 D1，关闭 Open Question Q1。）
- [x] 1.2 跑 `Skill(impeccable)` 视觉契约钩子（按 `.claude/rules/opsx-apply-cadence.md::Propose → Apply 之间` 第 2 项）：refine `design.md::Visual Contract` 段（Surface / Visual Layer / State Coverage / DESIGN.md delta plan），把关键视觉决策升级为 `D-V<n>` 编号写入 `## Decisions`。impeccable 产出若与现有 D-V1..D-V4 冲突，以 impeccable 输出为准并在 design.md 追加 changelog 备注。
- [x] 1.3 跑 codex design 二审（按 `.claude/rules/codex-usage.md` 第 3 节，UI 重构 + 新组件触发默认调）：`Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`，prompt 模板参考 `.claude/templates/codex-prompt-design-review.md`。返回 bug 修 design / spec / tasks 三处后 re-validate。
- [x] 1.4 `openspec validate replace-detail-cwd-with-meta-menu --strict` 通过（无新错误 / 警告，scenarios 4 hashtag 等格式合规）。

## 2. 组件实施 — `SessionMetaMenu.svelte`

- [x] 2.1 在 `ui/src/lib/icons.ts` 新增 `MORE_HORIZONTAL_SVG` lucide 多 path 常量（三 circle r=1，stroke-based 渲染为三圆点）。
- [x] 2.2 新建 `ui/src/components/SessionMetaMenu.svelte`，签名 `{ cwd: string | undefined; sessionId: string }`。组件内自管 `open: boolean` + `feedback: 'copied' | 'open-fail' | 'copy-fail' | null` + `feedbackTimer` state。
- [x] 2.3 trigger 渲染：自管 `.meta-trigger` 样式（与 `.top-badge` 同源 token：13px icon + padding 6px 8px + radius 6px）。trigger 含 `MORE_HORIZONTAL_SVG` icon，无 text label / pill。`aria-haspopup="menu"` + `aria-expanded={open}` + `aria-label="会话操作"`。
- [x] 2.4 menu overlay 渲染：trigger 下方 4px、**右对齐 trigger 右沿**定位（`top: r.bottom + 4px; right: window.innerWidth - r.right`），viewport 左边界 clamp（margin 8px）。容器 `role="menu" aria-orientation="vertical"`。实现 outside-click + `Esc` 关闭 + 方向键导航（disabled 项 SHALL 跳过）+ focus 回到 trigger。
- [x] 2.5 menu 项渲染：每项 padding `6px 12px`，font-size 13px；hover 态 background `surface-raised`，文本 `text`。
- [x] 2.6 实现「在文件管理器中打开」item（仅 `isTauriRuntime() === true` 渲染）：`import { openPath } from '@tauri-apps/plugin-opener'`，调 `await openPath(cwd)`；catch reject 切 `feedback = 'open-fail'` 并 `console.warn`。文案按 `navigator.platform` 分支：`Mac*` → `在 Finder 中打开`，其他 → `在文件管理器中打开`。
- [x] 2.7 实现「复制工作目录路径」item：调 `navigator.clipboard.writeText(cwd)`；resolve → `feedback = 'copied'`，reject → `feedback = 'copy-fail'`。
- [x] 2.8 实现「复制 Session ID」item：复制 `sessionId` 字符串，反馈状态同 2.7。Tauri mode 下第三项前渲染 1px `border-subtle` 分隔线；HTTP server mode 隐藏 Finder 项 + 不渲染分隔线。
- [x] 2.9 cwd 缺失（`undefined` / 空串）降级：「在文件管理器中打开」+「复制工作目录路径」两项渲染 disabled 态（`text-muted` / `cursor: not-allowed` / `aria-disabled="true"` / 不响应 click 与 Enter/Space）。
- [x] 2.10 trigger 反馈状态：trigger 下方 4px portal-style 浮 micro-toast（`position: fixed; z-index: 200`），文案分别为 `已复制` / `打开失败` / `复制失败`，fade-in 100ms / hold 1500ms / fade-out 150ms，setTimeout 切回 null。组件 onDestroy 清理待执行 timer。
- [x] 2.11 a11y：trigger `focus-visible` ring + `aria-haspopup="menu"` + `aria-expanded` + `aria-controls=<menu-id>`；menu 容器 `role="menu" aria-orientation="vertical"`；menu 项 `role="menuitem"`；分隔线 `role="separator"`；disabled 项 `aria-disabled="true"` 且 Tab 跳过（`tabindex="-1"`）。
- [x] 2.12 单测 `ui/src/components/SessionMetaMenu.test.svelte.ts`（命名约定与既有 `*.test.svelte.ts` 同源）：15 个 test 覆盖 trigger 渲染 + 平台分支 + cwd 缺失降级 + 三项操作 + plugin/clipboard reject 反馈 + 1500ms toast 自动消失 + ESC / 外部 click 关闭。

## 3. 集成 — `SessionDetail.svelte`

- [x] 3.1 删除现有顶 bar CWD markup（原 line 732-738，包含 `top-stat-sep · CWD top-stat-cwd top-stat-num`）。
- [x] 3.2 删除对应 CSS（原 line 1310-1325，`.top-stat-cwd` 与 `.top-stat-cwd .top-stat-num` 两条规则及上方注释）。
- [x] 3.3 `.top-stats` 规则改 `flex-wrap: nowrap`；保留 `gap: 7px`。
- [x] 3.4 在 `.top-meta` 区 `[Context]` button **左侧** 挂 `<SessionMetaMenu cwd={metaCwd} sessionId={sessionId} />`，cwd 通过 `{@const metaCwd = ...}` 从 `detail.metadata` 提取（typeof guard 兼容老 jsonl）。
- [x] 3.5 LAST 时间精度降级：在 `SessionDetail.svelte` 内新增 `ftimeMinutes(ts)` 局部 helper（用 `toLocaleTimeString` 仅 hour+minute，不动 `ftime` / `formatClock`，避免影响 chunk header / user meta time 等其他调用点）；line 729 `LAST` 处改用 `ftimeMinutes`。
- [x] 3.6 `pnpm --dir ui run check`（svelte-check）通过 0 error / 0 warning（既有 `Connection.svelte` warning 与本 change 无关）。

## 4. 测试与浏览器自验

- [x] 4.1 跑 `pnpm --dir ui run test:unit` 含新增 `SessionMetaMenu.test.svelte.ts` 全绿（15 tests passed；总计 421 passed + 1 skipped）。
- [ ] 4.2 playwright e2e `session-meta-menu.spec.ts`：跳过本 PR——vitest 14 个 scenarios 已覆盖 spec 主要 menu 行为；e2e 留作后续单独 PR（非阻塞，且改动不涉及 HTTP transport / IPC schema）。
- [ ] 4.3 `just test-e2e`：跳过（同 4.2）。
- [x] 4.4 IPC contract test：本 change 不改 IPC schema；`just preflight` 已跑 `cargo test --workspace`（含 `cdt-api ipc_contract` 测试套）全绿。
- [x] 4.5 浏览器手动自验：起 `pnpm --dir ui run dev` + chrome-devtools mcp 自动化截 4 张图（idle / menu open with cwd missing degrade / toast / dark theme）—— 视觉契约硬约束（`ui/CLAUDE.md::视觉改动自验`）已履行：① top-stats 单行 nowrap 无 CWD 长串、② menu 三项 + cwd 缺失前两项 disabled、③ trigger-anchored toast「已复制」right-anchored、④ 深色主题 token 切换正常。截图存 `/tmp/cdt-meta-menu-*.png`，PR 评论附引用。
- [ ] 4.6 桌面端 `just dev` smoke：移交 reviewer / merge 后人工验证（plugin-opener 真路径 + clipboard 真复制无法在 jsdom mock 中验证）。
- [x] 4.7 lint：`cargo clippy` + `cargo fmt --check` 通过（`just preflight` 子任务）。
- [x] 4.8 `just preflight` 一把梭过（fmt + lint + cargo test workspace + 421 vitest + 29 spec validate + IPC commands sync）。

## 5. 记录与同步

- [ ] 5.1 PR 描述写明 Perf impact `N/A（纯前端组件 + clipboard 调用，无 hot path / 无 IPC payload 变化）`，附手动 desktop smoke 验证清单。
- [x] 5.2 `proposal.md::Impact` 段：实施未引入 proposal 之外的新文件 / 字段，无需回补。
- [x] 5.3 `ui/CLAUDE.md::UI 组件规范` 不追加（本 change 未抽公共 `Menu.svelte`，第二个 menu 用例出现再抽）。

## N. 发布

- [ ] N.1 push 分支 + 开 PR（`gh pr create`，title 例：`feat(ui): SessionDetail 顶 bar meta-action menu 替换 CWD 显示`）
- [ ] N.2 wait-ci 全绿（`/wait-ci <pr>` 或后台 `gh pr checks <pr> --watch --fail-fast --interval 30`，与 N.3 并行启动）
- [ ] N.3 codex 二审通过（`Agent({ subagent_type: "codex:codex-rescue", ... })` PR 二审；如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

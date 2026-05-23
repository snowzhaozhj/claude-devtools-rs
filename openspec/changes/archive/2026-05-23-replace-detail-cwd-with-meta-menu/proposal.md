## Why

会话详情页顶 bar 当前直接渲染完整 `cwd` 长串（如 `/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.claude/worktrees/feat+keyboard-shortcuts`），与同行短量化指标（AI / USER / TOOLS / TOK / LAST）混排。实际症状与价值评估：

1. **视觉错配**：`top-stats` 设计 grammar 是 chip-like 量化指标（`font-mono 11px label` 短串），CWD 是变长字符串。`flex-wrap: wrap` + `flex-shrink: 1` 组合下 CWD 整段被踢到第二行，并在第一行末尾留下孤悬分隔符 `·`。
2. **主从颠倒**：长 mono 路径视觉重量超过 h1 主题——眼球先扫到路径再回扫 h1，违反 PRODUCT.md "信息密度高但不嘈杂"。
3. **信息冗余**：路径 95% 字符（home prefix / 个人项目根 / `.claude/worktrees/` 约定路径）信息熵 ≈ 0；唯一有信息熵的 worktree 名 sidebar 已经用 `#feat+xxx` 标签显示。
4. **未契约化**：`session-display` spec 1204 行无任何 cwd scenario，原版 TS `claude-devtools/src/renderer/` 也不渲染 cwd——本仓"加上去"的 patch 偏离原版且未走 spec。
5. **隐私**：截图分享 session 给团队时暴露 home 用户名 + 完整路径，与 PRODUCT.md "工程化、可信" 调性不符。

但完整删除会丢失「在 Finder 打开 cwd / 复制路径用作终端 cd」的低频但真实操作价值。本 change 的产品定位是：**把"显示路径"这个无价值常驻信息，置换为"对会话工作目录的 on-demand 操作入口"**。

## What Changes

- **删除** `SessionDetail.svelte` 顶 bar `top-stats` 行的 CWD chip（markup line 732-738 + CSS line 1310-1325）。
- **新增** `SessionMetaMenu.svelte` 组件：以 icon-only `[⋯]` 按钮形式挂 `top-meta` 区，与既有 `[Context]` 并列，复用 `.top-badge` 样式 token。点击展开 dropdown 菜单，含三项：
  1. `在 Finder 中打开`（macOS）/ `在文件管理器中打开`（其他平台）—— 调 `tauri-plugin-opener` 的 `open_path`；浏览器 / HTTP server mode 下隐藏该项。
  2. `复制工作目录路径` —— 写入 `navigator.clipboard`。
  3. `复制 Session ID` —— 写入 `navigator.clipboard`，与 1/2 项以分隔线分组。
- **顺手** 把当前 `LAST 19:50:46` 的秒级精度降级为分钟级（`19:50`），与 sidebar 「刚刚 / 18m / 1h」时间显示密度对齐。
- **不新增** terminal 一键打开能力——跨 OS + 用户偏好（Terminal.app / iTerm2 / Warp 等）需要独立 settings 项与失败处理，作为后续独立 change 评估。
- **`top-stats` 行**：移除 CWD 后保持 `flex-wrap: nowrap`（防止未来再有人加变长项触发同类 wrap 问题），并保留对未来 nominal 短指标项的扩展性。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-display`：新增 SessionDetail 顶 bar `top-meta` 区的 meta-action menu 行为契约；移除既有顶 bar CWD 文本显示约束（spec 原本即未约束，但 delta 显式记录"不再渲染顶 bar CWD"以对齐实现真相）。

## Impact

**前端代码**：
- `ui/src/routes/SessionDetail.svelte`：删 CWD markup + CSS，挂 `<SessionMetaMenu>`，`top-stats` 改为 `flex-wrap: nowrap`，`LAST` 时间格式调整。
- `ui/src/components/SessionMetaMenu.svelte`（新增）：dropdown menu 组件，复用 `Dropdown.svelte` 现有 affordance 或独立实现 menu（视设计稿决定，详见 design.md 决策表）。
- `ui/src/lib/icons.ts`：新增 `MORE_HORIZONTAL` lucide path 常量。
- `ui/src/lib/__fixtures__/`：mock fixture 新增 cwd 元数据覆盖（用于浏览器调试 + e2e）。

**plugin / IPC**：
- 复用既有 `tauri-plugin-opener`（已注册），无 IPC schema 改动；`navigator.clipboard` 浏览器原生 API 无依赖。

**测试**：
- vitest：menu 行为 + 平台分支（HTTP mode 隐藏 Finder 项）+ 复制成功/失败状态。
- playwright：详情页 [⋯] 点击 → menu 展开 → 各项点击行为（mock `tauri-plugin-opener` 调用断言）。
- 不影响 cdt-api IPC contract test（CWD 字段仍在 `detail.metadata.cwd` 数据通路保留）。

**spec / docs**：
- `openspec/specs/session-display/spec.md`：增 Requirement「SessionDetail 顶 bar meta-action 入口」+ scenarios；删除（或显式 NOT 化）历史可能存在的 cwd 渲染相关隐含约束。

**视觉契约**：
- 涉及新组件 `.svelte` + 改 `top-bar` 区——按 `.claude/rules/opsx-apply-cadence.md::Propose → Apply 之间` 钩子 SHALL 跑 `/impeccable shape`，产出 D-V 决策 + Visual Contract 段写入 design.md。

**对齐原版偏差**：
- 本 change 偏离原版 TS（原版无 cwd 顶部显示，本仓 detail meta-menu 是新交互）。`memory::feedback_align_with_original` 默认要求与原版对齐——本 change 显式记录"原版无此入口；本仓基于 cwd 数据已存在 + 用户实际操作需求新增 menu，不是为视觉而造"。

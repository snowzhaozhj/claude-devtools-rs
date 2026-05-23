## Why

长会话（数千 chunk）场景下，用户向上翻阅历史后想回到最新消息只能机械滚轮 / 拖滚动条，体感慢、易过冲。这是审计场景的高频痛点：用户在最新输出旁查 subagent / Bash 长输出时上滚 50 屏，看完想回去验证最新结果，**没有 affordance 提示"可以一次回最新"**。

`SessionDetail.svelte` 现状：
- 已有自动跟随（贴底 16px 内时新内容到达自动滚到底，`refreshDetail::wasAtBottom`）—— 不解决用户主动上滚后回最新
- 已有 `scrollAnchorIntoView(target)` smooth 滚到任意 DOM 元素的工具函数（被 ContextPanel / SearchBar 用）—— 但**没有"滚到最新"入口**
- 全局 keydown 仅拦 `Cmd+F`，不响应跨平台滚动快捷键

TS 原版（`src/renderer/components/chat/ChatHistory.tsx`）在距底 > 300px 时显示右下角浮动 `Bottom` 按钮 + Ctrl+R 触发跳底事件 —— 但其 `rounded-full` pill + `shadow-lg` + 标签文字风格与本仓 IDE / 调试器 register 不符，不能直接照搬。

## What Changes

- **session-display**: 新增 Requirement `Quick Anchor Navigation`，约束 SessionDetail 在 conversation 滚动距底 > 300px 时浮现"跳到最新消息"按钮 + 跨平台键盘快捷键（mac `⌘+↓` / Win+Linux `Ctrl+End`）触发 smooth 滚动到最末。按钮形态遵循 DESIGN.md `Buttons and icon controls` + `Border Before Shadow Rule` + `Status Owns the Color Rule`：icon-only 28×28、neutral surface-raised + emphasis border + 轻 elevation、不引入新色彩通道。

- **DESIGN.md** 同 PR 落 delta：
  - Section 5 `Components` 末尾新增 `### Floating affordances` 子节，记录浮层按钮契约（位置 / 形态 / 颜色 / Elevation / 进出 / States / A11y）
  - 新增 Named Rule `**The Floating Is Affordance, Not Decoration Rule.**` —— "浮层按钮仅在动作语义存在时显现；动作不再适用即隐去；不作为持久导航或装饰"，与 `The Persistent Selection Is Quiet Rule` 互补（持久 UI 安静；瞬时 affordance 浮起仅在它能完成的动作仍有意义时）

- **不在 scope**：
  - 回顶（⌘+↑ / Ctrl+Home / chevrons-up 按钮）—— 见 `design.md::Future Considerations`
  - End / Home 兜底键 —— 同上
  - "新内容到达"dot 徽章 —— 引入新色彩通道违反 Status Owns the Color Rule，按用户反馈再开
  - 多锚点导航（上一/下一条 user 消息）—— 由 ContextPanel phase 视图承担，不重复入口
  - `Cmd+Enter` 跳到末尾 user 输入 —— power user 路径，留 Future Considerations

## Impact

- Affected specs: `session-display`（ADDED Requirement: Quick Anchor Navigation）
- Affected code:
  - `ui/src/routes/SessionDetail.svelte`
    - 新增 `isFar` derived（`scrollTop + clientHeight < scrollHeight - 300`）
    - 新增 `isProgrammaticScroll` flag（点击 / 键盘触发 smooth scroll 期间抑制按钮显隐）
    - 扩展 `handleKeydown` 加 `⌘+↓`（mac）/ `Ctrl+End`（Win/Linux）拦截（input/textarea/contenteditable focused 时不拦）
    - conversation 滚动监听 + 单帧 rAF 节流
    - 浮动按钮渲染（inline 或抽 `JumpToLatestButton.svelte`，看代码量决定）
    - ContextPanel 打开时 `right` 让位逻辑（`right: contextPanelVisible ? PANEL_WIDTH + 16 : 16`）
  - 可能新增 `ui/src/components/JumpToLatestButton.svelte`（决定标准：≥ 80 行 inline 就抽）
  - `ui/src/lib/icons.ts`：新增 `iconChevronsDown` 常量（lucide path）
  - `ui/src/lib/platform.ts`：复用现有 `isMac()` helper（grep 确认；不存在则补）
- Affected tests:
  - 新增 `ui/src/routes/SessionDetail.test.svelte.ts`（或扩展已有）：
    - 距底 ≤ 300px 不显示按钮
    - 距底 > 300px 显示按钮
    - 点击按钮触发 smooth scrollTo(scrollHeight)
    - programmatic-scroll 期间不重显
  - 新增 `ui/tests/e2e/session-jump-to-latest.spec.ts`：
    - 长会话浏览器渲染下用户上滚 → 按钮浮现 → 点击回底
    - mac 端 `Cmd+ArrowDown` 触发跳底（emulate platform）
    - Win/Linux `Ctrl+End` 触发跳底
    - input focused 时键盘不拦截
    - reduced-motion 下进出降级为即时
- Affected docs:
  - `DESIGN.md`：Section 5 加子节 + 新 Named Rule
- Affected backend: 无（不动 IPC、不动 Rust crate）

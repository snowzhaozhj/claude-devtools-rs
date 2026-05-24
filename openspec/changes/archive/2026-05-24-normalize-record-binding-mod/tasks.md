## 1. platform.ts 工厂 + 迁移函数

- [x] 1.1 在 `ui/src/lib/platform.ts` 新增 `recordBindingFromEvent(event: KeyboardEvent): string | null` 工厂：先调 `normalize(event)` 得平台特化字符串，若结果为空（仅按下 modifier）返回 `null`；否则把当前平台主修饰键反写为 `mod`（mac `meta+` → `mod+`、win/linux `ctrl+` → `mod+`），返回 `mod+x` 字面量
- [x] 1.2 在 `ui/src/lib/platform.ts` 新增 `normalizeBindingToMod(binding: string): string` 迁移函数：把字面量中作为主修饰键的 `meta` / `ctrl` token 替换为 `mod`，幂等（已含 `mod` 不变）；不含主修饰键的 binding（`alt+x` / `shift+x` / `F1`）原样返回
- [x] 1.3 在 `ui/src/lib/platform.ts::formatMainKey` 给 `Space` 加 mac 平台分支返回 `␣`（U+2423），其他平台保持 `Space`

## 2. KeyRecorderInput Win 键守卫

- [x] 2.1 在 `ui/src/components/settings/KeyRecorderInput.svelte` 加内部 `let winKeyWarning = $state(false)`
- [x] 2.2 在 `handleKeyDown` 的 recording 态分支中，于调 `recordBindingFromEvent` **之前**加守卫：`if (!isMac() && event.metaKey) { winKeyWarning = true; return; }`
- [x] 2.3 把 `handleKeyDown` 中原有的 `normalize(event)` 调用替换为 `recordBindingFromEvent(event)`（保持 `null` → 继续等键、非空 → commit + blur 的分支语义）
- [x] 2.4 任意有效 commit 路径前 reset `winKeyWarning = false`（下次有效按键自动清除 warning）；`stopRecording()` 也 reset
- [x] 2.5 hint 区域文本：`hintText` derived 增加 warning 优先级——`if (winKeyWarning) return "Windows 不支持 Win 键作为修饰键，按目标组合键重新录入"`；优先级在 conflict 之前
- [x] 2.6 视觉态 class：`<div class:warning={winKeyWarning && !conflict}>`；CSS 走 `--surface-conflict-bg` / `--border-conflict` token（与 conflict 态相同视觉规约，复用 `DESIGN.md::The Conflict Is Warning Not Error Rule`）
- [x] 2.7 a11y：把 `aria-live="polite"` 显式声明到 hint `<span>`（line 152 附近），同步从 recorder 容器 div（line 134）移除该 attribute——避免 SR 在 focus / pressed / class 变化时对容器双宣告 noise，仅 hint 文本切换才是用户关心的语义

## 3. customization.ts bootstrap 迁移

- [x] 3.1 在 `ui/src/lib/keyboard/customization.ts::mergeOverrides` 中对每个 override binding 调 `normalizeBindingToMod` 迁移；保持既有"幽灵 ID 过滤" / "空串跳过"逻辑
- [x] 3.2 在 `registry.update(id, newBinding)` 入口（`ui/src/lib/keyboard/registry.ts` 或同等位置）调 `normalizeBindingToMod` 作为护栏，确保运行期 update 也走 mod 表达

## 4. nit

- [x] 4.1 `ui/src/lib/keyboard/register-app-shortcuts.ts` 顶部注释把"9 条 / 17 specs"改为"17 条（其中 tab.switch.1~9 占 9 条；外加 1 command-palette + 1 sidebar + 1 tab.close + 1 tab.next + 1 tab.prev + 1 pane.split + 1 pane.focus.next + 1 pane.focus.prev = 17）"，方便维护者校对
- [x] 4.2 `WIN_TEXT.meta = "Win"` 在 `ui/src/lib/platform.ts` 加注释说明"防御性 fallback；走 mod 归一后正常路径不触发"

## 5. 单测覆盖

- [x] 5.1 `ui/src/lib/platform.test.ts`（或新建）覆盖 `recordBindingFromEvent`：mac `Cmd+Shift+P` → `mod+shift+p`；win `Ctrl+Shift+P` → `mod+shift+p`；仅按 Cmd（无主键）→ `null`；mac `Cmd+Ctrl+X` → `ctrl+mod+x`（双修饰键边界）
- [x] 5.2 `ui/src/lib/platform.test.ts` 覆盖 `normalizeBindingToMod` ≥ 8 种字面量：`meta+x` / `ctrl+x` / `mod+x`（幂等）/ `alt+ctrl+x` / `shift+meta+p`（用户手编非 sorted）/ `meta+mod+x`（异常字面量归一为 `mod+x`）/ `ctrl+meta+x`（mac 双修饰键 sort 结果）→ `ctrl+mod+x` / `alt+x`（不变）/ `F1`（不变）
- [x] 5.3 `ui/src/lib/platform.test.ts` 覆盖 `formatShortcut` 在 mac 对 `Space` 输出 `␣`、win 输出 `Space`
- [x] 5.4 `ui/src/lib/keyboard/customization.test.ts`（或同名）覆盖 `mergeOverrides` 把存量 `meta+x` / `ctrl+x` 迁移为 `mod+x` + dispatcher 命中后 effective keymap 正确
- [x] 5.5 `ui/src/components/settings/KeyRecorderInput.test.ts`（或新建）覆盖 Win 键守卫：模拟 win 平台 + `Win+B` keydown，断言 `onCommit` 未被调用、warning 文本可被 testing-library 查到、第二次按非 metaKey 组合 commit 成功且 warning 清除；新增覆盖：`Win+B → Esc` 路径（warning 清除 + 退出 recording + `onCommit` 不被调用）；`Win+B → 仅按 Shift` 路径（warning 清除 + 保持 recording）；hint span 上 `aria-live="polite"` attribute 存在
- [x] 5.6 mac 平台覆盖：`Cmd+B` 走正常 commit 路径，断言 `onCommit` 收到 `mod+b`、warning 不触发；`Cmd+Ctrl+X` 走正常 commit 路径，断言 `onCommit` 收到 `ctrl+mod+x`

## 6. 验证

- [x] 6.1 `pnpm --dir ui run check`（svelte-check）
- [x] 6.2 `just test-ui-unit`（vitest 全跑）
- [x] 6.3 `cargo clippy --workspace --all-targets -- -D warnings`（兜底确认无 backend 误改）
- [x] 6.4 `openspec validate normalize-record-binding-mod --strict`
- [x] 6.5 `just preflight`

## N. 发布

- [x] N.1 push 分支 + 开 PR（PR body 含 `Closes #247`）
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

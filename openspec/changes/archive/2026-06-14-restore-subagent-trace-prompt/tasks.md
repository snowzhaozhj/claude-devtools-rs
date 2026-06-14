## 1. 前端逻辑：buildDisplayItemsFromChunks 产出 user_message

- [x] 1.1 `displayItemBuilder.ts` 从 `./toolHelpers` 引入 `cleanDisplayText` + `extractSlashInfo`，加一个 `userChunkText(content: string | ContentBlock[]): string` helper（string 直取 / Blocks 取首个 text 块）
- [x] 1.2 改 `buildDisplayItemsFromChunks`：`c.kind === "ai"` 走原逻辑；`c.kind === "user"` 时——`extractSlashInfo(raw) !== null` 跳过（slash），否则 `cleanDisplayText` 清洗后非空则 push `{type:"user_message", text, timestamp: c.timestamp}`；system / compact 仍跳过

## 2. 前端渲染：ExecutionTrace user_message 分支

- [x] 2.1 `ExecutionTrace.svelte` 新增 `{:else if item.type === "user_message"}` 分支：BaseItem + USER 图标 + summary 截断 + prose body（参照 SessionDetail.svelte user_message 渲染 + ExecutionTrace output 分支样式）

## 3. 测试

- [x] 3.1 `displayItemBuilder.test.ts` 加 `buildDisplayItemsFromChunks` 单测：UserChunk(prompt) → 产 user_message item（含文本 + timestamp）
- [x] 3.2 加 case：slash UserChunk（`<command-name>/x</command-name>`）→ 不产 user_message
- [x] 3.3 加 case：清洗后为空的 UserChunk（如纯 system-reminder）→ 不产 item；AIChunk 仍正常平铺
- [x] 3.4 反转验证：临时把 `c.kind === "user"` 分支改回 `continue`，跑 3.1 应 fail，确认测试真抓回归；再改回

## 4. 本地验证

- [x] 4.1 `pnpm --dir ui run check` 通过
- [x] 4.2 `just test-ui-unit`（或 `vitest --run displayItemBuilder`）通过
- [x] 4.3 浏览器 mock 或 `?http=1` 真数据展开一个 subagent trace，确认 prompt 显示在轨迹顶部、slash 不重复（视觉自验截图）

## 5. 发布
- [x] 5.1 push 分支 + 开 PR
- [x] 5.2 wait-ci 全绿
- [x] 5.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回 5.2；可循环）
- [x] 5.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

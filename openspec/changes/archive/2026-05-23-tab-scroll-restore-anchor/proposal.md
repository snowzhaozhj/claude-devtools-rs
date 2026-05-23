## Why

PR #223 修了「`onDestroy` 在 Svelte 5 element unmount 后触发 → `conversationEl.scrollTop` 永远读 0」这条路径，但留下了一条它误诊为 "scroll anchoring" 的 known limitation：**切回 tab 后 conversation 滚动位置与切走前对不上，尤其是滚到底部的场景**。

实际 root cause 不是 scroll anchoring，是 **lazy markdown 占位高度 vs 真实渲染高度的偏差 + scrollTop clamp 双重作用**：

- `lazyMarkdown.svelte.ts::estimatePlaceholderHeight` 按 80 字符 / 行 × 22 px 死算占位高度；highlight.js 行高、表格、code block、mermaid 等让真实渲染高度普遍高于估算
- 切回 cached path 时 `await tick()` 后所有 lazy-md 节点尚未触发 `IntersectionObserver` 回调，scrollHeight ≈ Σ 占位高度，远小于保存时的 scrollHeight
- 此时把 `scrollTop = savedValue` 写入会被浏览器 clamp 到 `max = scrollHeight - clientHeight`，底部场景偏差可达数千 px
- 后续 lazy chunks hydrate 让 scrollHeight 膨胀回原值，但 scrollTop 不再追随（CSS scroll anchoring 只对视口内已渲染元素位移生效，不补偿初始 clamp）

用户已经报过两次（PR #223 描述 "known limitation 留 followup" + 本轮反馈"会话没更新也对不上"），数值方案在 lazy render 架构下治标不治本。

## What Changes

把 `tabStore.svelte.ts` 的 `TabUIState` 滚动持久化字段从「绝对 scrollTop 数值」改为「视觉锚点 + 粘底语义」三件套：

- 新增 `atBottom: boolean`：保存时点是否粘底（distanceFromBottom ≤ 16）
- 新增 `anchorChunkId: string | null`：视口顶第一个完全可见的 chunk 的稳定 ID
- 新增 `anchorOffsetPx: number`：anchor chunk 相对 conversation 容器顶的像素偏移
- **BREAKING**：删除字段 `scrollTop: number`（per-tab 内存级状态，重启即清，不影响外部数据）

恢复策略：
- `atBottom=true` → `scrollTop = scrollHeight` + ResizeObserver pin 直到稳定 200 ms / 用户主动 scroll / 5 s 超时
- `atBottom=false` 且 `anchorChunkId` 命中 → `el.scrollIntoView({ block: 'start' })` + `scrollTop -= anchorOffsetPx`
- 兜底：anchor chunk 已被裁剪 / 找不到 → 降级到首屏顶部，记 console.warn

`SessionDetail.svelte`：拆 `captureScrollAnchor()` / `restoreScrollAnchor()` / `startBottomPin()` 三个函数；scroll listener 同步维护 `latestAnchor`（替代旧 `latestScrollTop`）；chunk 渲染处统一加 `data-chunk-id`。

`tab-management::滚动位置恢复` scenario 措辞从「恢复到之前保存的值」收紧为「按视觉位置恢复（粘底优先 / 否则锚点 + 偏移）」+ 新增三条 scenario 覆盖底部 / 中间位置 / lazy 占位极端场景。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `tab-management`：`滚动位置恢复` scenario 措辞收紧 + `TabUIState` schema 字段调整 + 新增 lazy render 协作 scenario

## Impact

- 代码：
  - `ui/src/lib/tabStore.svelte.ts`：`TabUIState` interface 字段调整，`getTabUIState` 默认值更新
  - `ui/src/routes/SessionDetail.svelte`：scroll save/restore 逻辑重写，chunk 渲染加 `data-chunk-id`
  - `ui/src/components/SessionDetail.test.svelte.ts`：vitest 覆盖 captureScrollAnchor 选锚点逻辑 + atBottom 阈值
  - `ui/tests/e2e/tab-scroll-preserve.spec.ts`：e2e 三场景重写（底部 / 中间 / lazy 全占位）
- 跨域：`ui` 单点；不动后端 / IPC / Tauri 配置
- 无依赖变化、无外部协议变化、无性能预算（涉及 ResizeObserver 但仅在恢复期短窗口内活跃）
- 与 PR #223 的 known limitation followup 关闭对应（archive 时移除留存条目）

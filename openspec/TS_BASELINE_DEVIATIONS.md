# TS Baseline Deviations

本文件装**端口期 baseline cross-check 发现的 TS 偏差预警** + **baseline 外的 UI 隐式契约**。

> **历史**：原名 `followups.md`，2026-05-23 改造为本文件。原文件混装了 (a) TS 偏差预警、(b) main 既有 bug、(c) 已修条目历史索引、(d) 路线图候选——文件涨到 712 行不可读。改造后：
> - main 既有 bug / coverage-gap / 跨 capability 长期项 → **GitHub Issues**（issue #230-#239 等）
> - 已修条目 → 删除（git log + archive + 主 spec 是真相源）
> - 路线图候选 → `openspec/README.md::路线图`
> - 本文件只留 **TS 偏差预警 + UI 隐式契约 + 暂不开 issue 的 backlog**

## 维护规约（硬约束）

1. **archive 一个 change 时**，SHALL 同步删除本文件内对应已修条目（避免再次膨胀到 700 行）
2. **新发现的 main 既有 bug** → 走 GitHub Issue（默认 `bug` label），**不**追加进本文件
3. **新发现的 TS 偏差预警**（port 期决策的有意偏离）→ 进本文件 `## TS deviations` 段
4. **新发现的 baseline 外 UI 隐式契约** → 进本文件 `## Implicit contracts` 段
5. 文件**应**保持 ≤ 200 行；超过说明 issue 分流不到位

## 图例

- **deviation**：Rust port 与 TS 原版有意偏离的实现选择（已决策不复刻 TS 行为）
- **spec-gap**：spec 描述不准确或与实现机制描述不一致（行为可一致）
- **implicit**：无法写进 baseline 的隐式契约（UI 交互、状态动画、键盘绑定等）

---

## TS deviations

### [deviation] is_interrupt_marker 对 array content 前导空白容忍——原版不 trim

- 代码：`crates/cdt-parse/src/noise.rs::is_interrupt_marker` 调用 `extract_user_text` 拼接所有 Text block 后 `text.trim().starts_with(INTERRUPT_PREFIX)`
- 原版差异：`claude-devtools/src/main/types/messages.ts:201-205` array content 单 Text block 含 `[Request interrupted by user` 起首的 interruption 判定**不**做 trim；string content 才 trim
- 后果：array 单 Text block 带前导空白 + interrupt prefix 起首的消息，本仓归 `MessageCategory::Interruption`（messageCount 不计入），原版仍计入 `isParsedUserChunkMessage`
- 触发概率：极低（前导空白 + interrupt 是边缘场景，CLI 写 JSONL 时不会带前导空白）
- 修法：要么改 cdt-parse 的 `is_interrupt_marker` 区分 string / array branch trim 行为；要么在 cdt-api `is_user_chunk_message` 层重判
- 来源：codex 二审 PR #38 sidebar-meta-row-fix change 第三轮 review

### [deviation] Rust 匹配 `Task || Agent` 两个工具名，原版只匹配 `Task`

- `resolver.rs` 的 Task filter 包含 `name == "Task" || name == "Agent"`
- 原版只有 `name === "Task"`
- **原因**：Claude Code 新版本把 Task 工具改名为 Agent，Rust port 做了兼容；非 bug，但需在 spec 里显式声明"Task/Agent 同义词"

### [deviation] image asset blockId 仍按 message uuid 寻址，duplicate uuid 取首条

- `ui/src/routes/SessionDetail.svelte::uimages` 生成 `blockId = "<chunk.uuid>:<idx>"`，后端 `cdt-api/src/ipc/local.rs::find_image_block_in_messages` 用 `messages.iter().find(|m| m.uuid == chunk_uuid)` 取**首条**同 uuid 消息
- 在 change `unique-chunk-id-non-ai` 触发的 duplicate-uuid 场景（`claude --bg` 启动时 bg session 把初始 prompt 回放到主 session JSONL）下，两条同 uuid 消息 content 字节级一致（已实测），`find` 取第一条不会拿错图——**当前真实数据下不是 bug**
- 但若未来上游 JSONL 出现"两条同 uuid 但 content 不一致"的极端情况，第二个 UserChunk 会加载第一个 chunk 的图。spec 上的"歧义"由 codex CR PR #116 第二轮二审标出
- 决策：**不阻塞 release**。`expandedCompacts` 是 per-chunk-instance UI state 改 `chunkId` 是必要的；`image asset` 是 per-message-content（图片资源跟随消息身份 uuid，而非 chunk 渲染分组）改不改 chunkId 都对真实数据无影响
- Followup（非阻塞，等真实场景触发后再决策）：若发现 content-divergent duplicate uuid 案例，把 blockId 编码切到 `chunkId:index` 并在后端按 chunkId 重新反查消息位置

---

## Implicit contracts（baseline 外，UI 层）

下列行为无法冻结进 baseline specs，Rust 重写选 UI 技术栈时需要单独决策是否复刻：

- **滚动编排**（`useTabNavigationController`, auto-scroll bottom, scroll restore）
- **搜索高亮跨会话定位**（`SessionSearcher` + 滚动联动 + 高亮持久化）
- **Tab 导航与关闭历史**（`tabSlice` + `tabUISlice`，每 tab 独立 UI 状态隔离）
- **键盘快捷键**（`keyboardUtils`，Tab 切换、搜索焦点、复制）
- **Markdown 渲染细节**（`react-markdown` + `remark-gfm` + `mermaid` + 代码块 syntax highlight）
- **主题切换与 CSS 变量级联**（`useTheme`，dark/light）
- **Dashboard 水瀑图渲染策略**（`waterfall` 数据 → 渲染形态）
- **虚拟滚动 / 大会话渲染性能**（decision on list virtualization 策略）
- **Notification 桌面提醒 / 系统托盘** 行为

这些条目在 Rust port 里属于 **UI 技术栈决策域**，可以按新栈习惯重做，不强制 1:1。

---

## Backlog（暂不开 issue，等真实痛点触发再决策）

下列条目优先级低 / 已 punt / 影响面窄，先记录在此；真有用户痛点或被维护者 grooming 时再升级为 GitHub Issue：

| 条目 | 性质 | 说明 |
|---|---|---|
| 多 tool 链接 / orphan tool_result / Task 过滤没有测试 | coverage-gap | `cdt-analyze` 内部，影响面窄；下次改这块时顺手补 |
| SSE 增量补全 ssh-status / updater 事件源 | coverage-gap | 桌面端走 `app.emit` 不受影响，仅 headless / 浏览器客户端缺；待 server-mode 用户报告 |
| SSH stage-limit 快速搜索未进 spec | coverage-gap | `SessionSearcher.ts:29-31 SSH_FAST_SEARCH_STAGE_LIMITS` 行为未冻结到 spec；下次改 session-search 时一并 |
| lazy markdown 副作用：浏览器原生 Cmd+F 不命中未渲染 chunk | coverage-gap | 已 punt（应用内 SearchBar 接管，原生 Cmd+F 极少用） |
| Windows 路径含非良构 UTF-16 时 normalize 误判 | coverage-gap | 极低概率（Claude CLI / 用户 typed 都是良构），待真实报告再处理 |
| WKWebView smart-select / Ctrl+Click / trackpad 双指 tap 桌面手测未跑 | implicit | CI 限制，需 macOS 真窗口手测；发版前 smoke 兜底 |
| Shift+F10 在 macOS 系统快捷键冲突未确认 | implicit | a11y 兜底，需桌面手测；若占用考虑 Cmd+Shift+M 替代 |
| HMR `import.meta.hot.dispose` 钩子未自动化测试 | implicit | dev-only 路径，单 process vitest / prod-build Playwright 都不复现；改动 contextMenu.svelte.ts 时手测 |
| `KeyRecorderInput.svelte::recorder-spin` 0.9s → 1.2s（与 DESIGN.md `The Recorder Idle State Rule` 引用的 1.2s 标准对齐）| visual-consistency | 1 行修法 + 同步 timing 单测；下次 touch `KeyRecorderInput.svelte` 时一并 |

---

## 已迁出本文件的内容

| 类型 | 去向 |
|---|---|
| main 上的 impl-bug + coverage-gap（13 个）| GitHub Issues #230-#239、#246-#248 |
| 已修条目历史索引 | 删除（git log / archive / 主 spec 是真相源） |
| Phase 2/3/4 telemetry 路线图 / Phase 1.5 micro-task / cdt-telemetry deferred | `openspec/README.md::路线图` |
| 性能 phase 1-5 优化叙事 | 删除（archive slug + `git log --grep="feat(perf)"` 是真相源） |

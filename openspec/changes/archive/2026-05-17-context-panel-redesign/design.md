## Context

`crates/cdt-analyze/src/context/session.rs::ai_chunk_id` 用 `responses[0].uuid`（fallback `ai-<turn_index>`）作为 `ContextInjection.aiGroupId`。而 `crates/cdt-analyze/src/chunk/builder.rs::next_ai_chunk_id` 给 `AIChunk` 分配的 `chunk_id` 形如 `ai:<uuid>:<n>`。**两份 ID 不字节级相等**，前端 ContextPanel 拿到 `aiGroupId` 后无法直接用它在 `chunks[].chunkId` 里找到对应 AIChunk。

前端 `ui/src/lib/contextExtractor.ts::injectionToEntry` 进一步把后端 6 类丰富字段（每类带 `turnIndex` / `aiGroupId` / `toolBreakdown` / `thinkingTextBreakdown` 等）压扁成 `ContextEntry { label, preview, estimatedTokens, path? }`，丢失了所有 turn 锚点与 breakdown 信息。结果 `ContextPanel.svelte` 只能渲染 6 类一律的 `<div>label preview tokens</div>`，用户在面板里点开看不到"哪个 turn 哪个 tool 贡献了多少 token"，也无法跳回正文。

TS 原版 `SessionContextPanel` 有 6 个独立 Section 模板（详见 `../claude-devtools/src/renderer/components/chat/SessionContextPanel/`），每个 Section 用专属布局；`onNavigateToTurn` / `onNavigateToTool` / `onNavigateToUserGroup` 三个回调把 ContextPanel 与 chat 视图打通；Phase Selector 在 Header；CLAUDE.md 分 Global / Project / Directory 三组。本 change 把这套移植过来，前提是先把 `aiGroupId` 与 `chunkId` 对齐。

## Goals / Non-Goals

**Goals:**

- 前端能用 `aiGroupId` 字节级精准定位到 `AIChunk` 的 DOM 节点（`data-chunk-id` 属性）
- 6 类 ContextInjection 各有专属 Section 模板，呈现关键字段（turn / tool / scope / breakdown）
- 点击任意 injection SHALL 滚动到 SessionDetail 中对应 AIChunk；点 tool breakdown 单条 SHALL 滚到具体 tool 子节点
- ClaudeMd Section 分 Global / Project / Directory 三组（对齐 TS 原版语义）
- 多 phase 会话 SHALL 提供 Phase Selector 切换查看不同 phase 的 injections
- Ranked 视图保留并加 Grouped/Flat 子切换

**Non-Goals:**

- 不改 `chunk-building` 算法或 `chunk_id` 生成规则
- 不引入新的 IPC command（aiGroupId 是同字段语义升级，不破协议）
- 不重写 Sidebar / TabBar / Settings / DashboardView 等其它视图
- 不动后端 `ContextStats` / `ContextPhaseInfo` 数据结构（前端消费 shape 不变）
- 不重构 DirectoryTree（保留现有实现，只是从一个 Section 内调用）

## Decisions

### D1：`ai_chunk_id` 改为直接复用 `AIChunk.chunk_id`，移除 uuid/fallback 二选一逻辑

**选择**：把 `cdt-analyze::context::session::ai_chunk_id(ai, turn_index)` 改为 `ai.chunk_id.clone()`，删除 `responses[0].uuid` 与 `ai-<turn_index>` 两条 fallback。

**理由**：
- 单一信息源原则——`AIChunk.chunk_id` 由 `chunk/builder.rs::next_ai_chunk_id` 统一生成，已经做了"`ai:<base>:<n>` + 全局 `used_chunk_ids: HashSet<String>` collision-free 兜底"（PR #114 引入 + PR #116 codex 二审加固）。复用它意味着 `aiGroupId` 自动继承这层兜底：哪怕 compact/replay 让 base uuid 重复，递增后缀 `n` 保证 `aiGroupId` 全局唯一，**不会**重新触发 PR #113 修过的 Svelte `{#each ... (key)}` duplicate-key 崩溃。
- 前端不需要做"`aiGroupId → chunkId` 映射"这种间接层，直接 `document.querySelector(\`[data-chunk-id="\${aiGroupId}"]\`)` 就能定位。
- 现有 `ai-<turn_index>` fallback 路径几乎不会触发（AIChunk 一定有 response；空 response AIChunk 在 chunk pipeline 已被丢弃），删掉它降低圈复杂度。

**候选**：
- A：前端维护 `aiGroupId → chunkId` 映射 hash。否决——多一层间接、需要在 SessionDetail 每次 detail load 重建映射；逻辑分散到前后端两侧，aiGroupId 一变前端就裂。
- B：保留 fallback，仅在前端做 prefix tolerant 匹配（去掉 `ai:` 前缀比对 uuid 中段）。否决——脆弱字符串解析，违反 "Don't add error handling for scenarios that can't happen"。

**风险**：
- 所有 `crates/cdt-analyze/tests/context_tracking.rs` 里断言 `aiGroupId == "ai-0"` 或具体 uuid 的测试都要更新。**Mitigation**：grep `ai_group_id` 一次性扫到全部断言点；改测试同步走在同一 commit 里。

### D1b（修订 D1）：彻底统一 `chunkId` 形态为 `<base>:<n>`，删 `ai:` 前缀 + 删 "首次裸 uuid" 特例

**触发**：用户在 apply 阶段质疑"`ai:` 前缀的存在价值"。审计后发现：
- `chunk_id` **不持久化**——前后端 grep `localStorage` / cdt-config / src-tauri 0 hit；`expandedChunks` 在 `tabStore.svelte.ts` 是 in-memory `Set<string>`，重启即丢；后端每次解析 JSONL 现算
- 既然全局 `used_chunk_ids: HashSet` 跨类型共享并做 collision-free 兜底，`ai:` 前缀的"namespace 隔离"作用是 dead design
- 原 D1 复用 `ai:<base>:<n>` 格式只是"不动现状"，没有论证形态本身合理性——这是偷懒不是彻底解法

**选择**：
- `builder.rs::next_ai_chunk_id` 与 `next_non_ai_chunk_id` 合并为单一 `next_chunk_id(base, used_set) -> String`，永远返回 `format!("{base}:{n}")`（n 从 0 起，撞了递增）
- 所有 chunk 类型首次出现都用 `<base>:0`（不再有"首次裸 base"特例）
- AI base = `responses[0].uuid` 或 fallback `"empty"`；non-AI base = 自身 `uuid`
- chunk 类型由 `Chunk::kind` 字段区分，**不**靠 chunkId 字面前缀

**理由**：
- 单一形态消除"首次 vs 后续"分支判断，简化代码逻辑（builder.rs 减 25 行）
- 跨类型一致让未来加 chunk type（SubagentChunk / ThinkingChunk）零决策成本
- collision-free 不依赖前缀——全局 set 已兜底，前缀是冗余的过度防御
- chunkId 既不持久化也不跨进程，**没有任何兼容性问题**——用户升级版本后，所有 chunk 都用新规则现算

**候选**：
- A：保留 `ai:` 前缀维持"调试可读性"。否决——`Chunk::kind` 字段已提供类型信息，log 里同时打 `kind=ai chunk_id=abc:0` 比 `chunk_id=ai:abc:0` 信息密度更高
- B：non-AI 保留"首次裸 base"（B1）。否决——同样是不一致 special case，长期维护成本 > 短期改动收益
- C：拆独立 PR 做。否决——D1 决策本身就要论证 chunkId 形态合理性，混进本 PR 不破"单一聚焦"（本 change 已 modifies 多个 capability），拆出去反而要等 merge 再 rebase

**风险**：
- 多处 fixture / 测试 / UI mock 字面值需改 → 机械修改，cargo test --workspace 自动覆盖（实测 30+ test suite 全绿）
- spec delta `ipc-data-api` MODIFIED "Stable chunk identifiers" Requirement → 已写完，含 6 个 scenario 覆盖统一格式 + 跨类型 + 撞车 corner case

### D2：ContextPanel 拆 6 个 Section 子组件，每个独立 `.svelte` 文件，放在 `ui/src/components/contextPanel/`

**选择**：
```
ui/src/components/contextPanel/
├── ContextPanelHeader.svelte
├── UserMessagesSection.svelte
├── ClaudeMdFilesSection.svelte
├── MentionedFilesSection.svelte
├── ToolOutputsSection.svelte
├── TaskCoordinationSection.svelte
├── ThinkingTextSection.svelte
├── PhaseSelector.svelte
└── CollapsibleSection.svelte   # 共享折叠头部
```
`ContextPanel.svelte` 作为 orchestrator，负责数据分发 + viewMode 切换 + scroll 调度，不再写具体 Section 样式。

**理由**：
- TS 原版正是这样组织（一对一移植），降低跨仓对比成本。
- 单文件大爆款 svelte 难 review；按 Section 拆开，未来加新 category 或改单个 Section 模板影响半径最小。
- 共享的 `CollapsibleSection` 把 header + chevron + token 计数 + 折叠状态封装一次，避免 6 处复制粘贴。

**候选**：
- A：所有 Section 留在单个 `ContextPanel.svelte` 内。否决——已经 540 行；6 类一进 if/else 分支再加上各类自己的 sub-collapse 会爆。
- B：完全按 TS 原版的"items + components 双层"拆。否决——粒度过细（10+ 文件），Svelte 不像 React 那样每个 component 都要 props drilling，单层 Section 内联各种 row 即可。

### D3：turn 锚点导航走 `data-chunk-id` DOM 属性 + `scrollIntoView`，不引 store 不绑 ref

**选择**：
- `SessionDetail.svelte` 渲染每个 `chunk` 容器时加 `data-chunk-id={chunk.chunkId}`；AIChunk 内每个 tool 子节点加 `data-tool-use-id={exec.toolUseId}`
- ContextPanel 各 Section 收 `onNavigateToChunk(chunkId)` 与 `onNavigateToTool(chunkId, toolUseId)` 两个回调；回调实现在 SessionDetail
- **统一调度顺序**（两类锚点共用一条 helper）：
  1. 若目标 `chunkId` 不在 `expandedChunks` 中，先 `expandedChunks = new Set([...expandedChunks, chunkId])`（必须新建 Set 触发 Svelte 5 响应式）
  2. `await tick()`（一次足够；Svelte 5 `tick()` 保证当前组件树所有 pending 更新已 flush 进 DOM——无需 double-rAF）
  3. `root.querySelector(\`[data-chunk-id="\${chunkId}"]\`)?.scrollIntoView({ block: "center", behavior: "smooth" })`
  4. 若需定位 tool：再次 `await tick()` 后 `root.querySelector(\`[data-tool-use-id="\${toolUseId}"]\`)?.scrollIntoView({ block: "center", behavior: "smooth" })`

**理由**：
- DOM 属性查询是 0-依赖、0-store 状态的方案，跟 Svelte 5 runes 重渲染语义无关
- 不需要给每个 chunk 维护 `bind:this`（chunk 数可达几千，bind 数组会让 Svelte 重渲染开销线性增长）
- `scrollIntoView` 原生支持 smooth + center，无需自实现
- TS 原版也是同样模式（DOM id-based scroll）
- `await tick()` 而非 `setTimeout`：Svelte 5 `tick()` 是组件级 promise，比 `setTimeout(0)` / `requestAnimationFrame` 语义更准确——前者保证组件状态变更已写入 DOM，后两者只是浏览器调度时机

**候选**：
- A：用 `IntersectionObserver` + chunkId → element map store。否决——重，需要在 chunk mount/destroy 维护 map，Svelte 5 destroy 顺序与 mount 不严格对称。
- B：URL hash + 浏览器原生锚点。否决——会污染 URL，且 Tauri webview 内 hash change 没有用户预期。
- C：`setTimeout(0)` 替代 `tick()`。否决——浏览器调度无保证；测试环境 fake timer 下不可控。

**风险**：
- 用户折叠了目标 chunk 时滚过去看不到内容。**Mitigation**：步骤 1 已显式 `expandedChunks.add(chunkId)` 再 scroll。
- AIChunk 默认 tools 折叠，点 toolUseId 时 tool 节点不存在。**Mitigation**：步骤 4 在第二次 `tick()` 后查 tool 节点；展开 tools 由 `expandedChunks` 控制，单次 add 即生效。

### D4：ClaudeMd 三组分类用 scope 字段，不引新数据

**选择**：`ClaudeMdFilesSection` 内部按 `scope` 分组：
- Global: `scope === "enterprise" || scope === "user"`
- Project: `scope === "project"`
- Directory: `scope === "directory"`

每组用 sub-header（"Global"/"Project"/"Directory"）+ DirectoryTree（已有组件）；空组不渲染。

**理由**：后端 `ClaudeMdContextInjection.scope` 字段已存在（`enterprise`/`user`/`project`/`directory` 四值），直接消费；TS 原版 `CLAUDE_MD_GROUP_CONFIG` 是同样的映射，但 TS 区分得更细（`user-memory`/`user-rules`/`auto-memory` 等）。本仓后端只产出 4 个粗 scope，前端按 4 → 3 折叠即可，不再扩 scope 枚举（避免 ripple 到 cdt-config 的 ClaudeMdScope 持久化逻辑）。

**候选**：
- A：扩 `ClaudeMdScope` 加 `user-memory` / `auto-memory` / `enterprise` 细分。否决——后端 `cdt-config::claude_md` 当前只产 4 scope，扩这个会拖动 `configuration-management` capability，超出本 change 范围。
- B：把 enterprise/user 当独立两组（Enterprise/User/Project/Directory 四组）。否决——一般用户没有 enterprise CLAUDE.md，四组里有一组永远空；合并 Global 更对齐 TS 原版的视觉密度。

### D5：Phase Selector 当且仅当 `phases.length > 1` 时显示，默认选 latest

**选择**：
- `ContextPhaseInfo.phases` 长度 ≤ 1 时 SHALL NOT 渲染 selector（避免无 phase 切换的会话出现孤零零下拉）
- 默认 `selectedPhase = null`（latest），下拉里有 "Latest"（=null） + "Phase 1" / "Phase 2" / ... 项
- 切换后 ContextPanel 只展示该 phase 范围内的 injections——通过 `phase_info.ai_group_phase_map` 把每个 injection 的 `aiGroupId` 映射回 `phaseNumber`，过滤匹配项

**理由**：保持视觉简洁；多 phase 会话才暴露 advanced control。

**候选**：
- A：始终显示 selector，单 phase 会话也显示 "Phase 1"。否决——95% 会话只有 1 phase（compact 不常触发），多一个空操作的下拉浪费视觉空间。

### D5b（修订 D5）：靠 `injectionsByPhase: Map<phaseNumber, ContextInjection[]>` 透传，不靠前端 `ai_group_phase_map` 反查

**触发**：codex 二审指出原 D5 假设"前端拿到 latest accumulated + ai_group_phase_map 就能反查 Phase 1 的 injections"——但 `cdt-api/src/ipc/local.rs::get_session_detail` 当前**只**填 `phases.last().last_ai_group_id` 的 accumulated_injections 进 `SessionDetail.contextInjections`，Phase 1 的 injections 在 compact 后 reset 时已被清空（`session.rs:95 accumulated_injections.clear()`），前端拿到的数组里**根本不含**Phase 1 的内容。原 D5 不可实现。

**选择**：`SessionDetail` IPC 新增两个 additive 字段：
- `phaseInfo: ContextPhaseInfo`（phases + ai_group_phase_map + compaction_token_deltas + compaction_count；前端按 phases.length > 1 决定 selector 显隐）
- `injectionsByPhase: Map<phaseNumber, ContextInjection[]>`（key = `phaseNumber.toString()`；每 phase 一份完整 accumulated_injections，从 `stats_map[phase.last_ai_group_id].accumulated_injections` 取）

前端：
- `selectedPhase = null`（Latest）→ 显示 `injectionsByPhase[最大 phaseNumber]`（即原 `contextInjections`，保留旧字段以兼容老前端 / 不破 round-trip 测）
- `selectedPhase = N` → 显示 `injectionsByPhase[N]`
- ContextPanel Header `Visible: ~Xk tokens` SHALL 显示**当前过滤后**的 token 总和（Latest 时即原行为；选中具体 phase 时只算该 phase 内的）

**理由**：
- Phase 1 完整 accumulated 必须由后端 IPC 传出来——前端没法从 latest accumulated 反推已被 reset 掉的内容
- additive 字段不破现有 IPC 契约（`contextInjections` 字段语义不变；旧前端忽略新字段）
- 后端 stats_map 已有 phase 末 last_ai_group_id 的 backfill 完整 accumulated（`session.rs:74-77 backfill`），新字段是**已计算结果的镜像**，不需要新算逻辑

**候选**：
- A：保留原方案，前端反查 ai_group_phase_map 过滤 latest accumulated。否决——Phase 1 数据根本没进 latest accumulated，反查无源。
- B：`contextInjections` 改为 `Map<phaseNumber, ContextInjection[]>`，breaking change。否决——会让所有现存 fixture / api 类型 / round-trip 测同时失败，扩散面太大。

**风险**：
- IPC payload 可能因每 phase 独立列表而增大。**Mitigation**：phases 是 reset 关系无重叠，总量等于"全会话所有 injection"——比当前"latest accumulated"仅多了被 compact 前的 Phase 1 数据，量级与现存 chunk payload 比可忽略。
- 旧前端拿不到 `phaseInfo` / `injectionsByPhase` 时不能崩。**Mitigation**：UI api.ts 字段都用 `?:` optional + 默认值；ContextPanel 单 phase 路径不读这两字段。

### D6：Ranked 视图保留并加 Grouped/Flat 子模式

**选择**：
- Grouped（默认）：按 category 颜色分块，块内按 token 降序
- Flat：所有 injection 平铺，按 token 降序，颜色 chip 留在左侧
- 子切换按钮放在 Ranked 视图顶部，与 Category/Ranked 主切换分两层

**理由**：对齐 TS 原版；Grouped 是当前实际默认（保留），Flat 是新增能力。

### D7：所有 Section 默认全部展开

**选择**：组件初始化 `expandedSections = new Set([ALL_6_SECTION_IDS])`。

**理由**：对齐 TS 原版（`useState(new Set([SECTION_USER_MESSAGES, SECTION_CLAUDE_MD, ...]))`）；用户打开 panel 就能直接看到内容，无需点击折叠头。空 Section（如无 mentioned file）不渲染整个组件，所以"全部展开"不会带来视觉噪声。

## Risks / Trade-offs

- **[Risk] `aiGroupId` 字段值变化** → 所有引用旧值 `ai-<turn_index>` 或裸 `uuid` 的测试与 fixture 失败。
  - **Mitigation**：D1 决策落地时 grep `ai_group_id`、`aiGroupId` 全部断言点，单次 commit 同步改完；ipc_contract test 校验新值 shape（`ai:<uuid>:<n>` 正则）。
- **[Risk] `data-chunk-id` 全 DOM 扫描查询性能** → 大会话有几千个 chunk，querySelector 一次 O(N)。
  - **Mitigation**：只在 navigate 触发瞬间查一次（用户点击事件），不是热路径；现代浏览器 querySelector 在几千节点上 < 10ms 可接受。
- **[Risk] 折叠/未展开的 chunk 无法 scroll 到子节点** → tool 锚点滚过去看不到内容。
  - **Mitigation**：D3 已说明——navigate 前先 `expandedChunks.add(chunkId)` 再 `await tick()` 再 query。
- **[Risk] 6 个 Section 拆分增加文件数** → 项目目录看似复杂。
  - **Mitigation**：所有新文件都放在 `ui/src/components/contextPanel/` 子目录，对外只 export `ContextPanel`，使用方无感。
- **[Risk] Phase 切换过滤掉 injections 后视觉很空** → 用户以为 panel 坏了。
  - **Mitigation**：空 phase 时 Header 仍显示 phase 信息 + "本 phase 无 injection" 占位文案；空 Section 仍按现有逻辑不渲染。
- **[Trade-off] CLAUDE.md scope 不细分到 enterprise/user 两组** → 与 TS 原版有一处语义差。
  - **接受**：本仓后端 scope 是 4 值粒度，强求 5 值会触发 `cdt-config` 改动，与本 change 主线无关；Global 合并 enterprise + user 视觉与 TS 原版基本一致。
- **[Trade-off] 不引 IntersectionObserver 做"当前可见 chunk 高亮"** → ContextPanel 无法显示"当前正看到哪个 turn"。
  - **接受**：超出本 change "解决看不明白" 的核心问题；可在后续 change 扩展。
- **[Risk] D5b 新增 IPC 字段需同步 6 处契约**（`crates/cdt-api/src/ipc/types.rs::SessionDetail` + `crates/cdt-api/src/ipc/local.rs::get_session_detail` 填充 + `crates/cdt-api/tests/ipc_contract.rs` round-trip + `ui/src/lib/api.ts::SessionDetail` 类型 + `ui/src/lib/__fixtures__/multi-project-rich.ts` fixture + `src-tauri/src/lib.rs` 透传）。
  - **Mitigation**：tasks.md 拆 6 条独立 checkbox 强制覆盖；新字段 `#[serde(default)]` + `Option` 让老 fixture 自动通过。

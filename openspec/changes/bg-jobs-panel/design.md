## Context

`claude --bg` 后台任务的状态存储在 `~/.claude/jobs/<job_id>/state.json`。每个 job turn 结束后覆写一次（频率分钟级）。目前只能通过 CLI `claude agents` 查看。

现有 FileWatcher 已监听 `projects/` 和 `todos/` 两个目录，通过 broadcast channel 推事件。扩展 `jobs/` 是同一模式的第三路。

IPC 侧 `LocalDataApi` 已有 `list_sessions` / `list_projects` 等 command，新增 `list_jobs` 遵循相同 facade 模式。

## Goals / Non-Goals

**Goals:**
- 只读展示 `~/.claude/jobs/*/state.json` 的实时状态
- FileWatcher 事件驱动推送（非轮询）
- TitleBar badge 显示"需要你操作"的任务数
- 分组对齐 `claude agents` 原生语义
- Session 跳转（跨项目可行）
- 降级健壮（jobs/ 不存在 → 零 UI 暴露）

**Non-Goals:**
- 不读 timeline.jsonl（数据源只有 state.json）
- 不提供 stop/kill 操作（Phase 2）
- 不主动 create_dir_all（jobs/ 不存在时不建目录）
- 不做 sidebar 分区或 dashboard 独占
- 不做 SSH 模式下的 jobs 展示

## Decisions

### D1：FileWatcher 扩展方式——加 `jobs_dir` 字段 + 第三路 broadcast channel

**选择**：在 `FileWatcher` struct 加 `jobs_dir: PathBuf` + `jobs_tx: broadcast::Sender<JobChangeEvent>`，`start()` 里 `is_dir()` guard + `watcher.watch(&jobs_dir, Recursive)`，`route_event` 加第三分支。

**替代**：独立 `JobsWatcher` struct。

**理由**：共用一个 `notify::RecommendedWatcher` 实例更节省 fd/内核资源；jobs/ 的路由逻辑（只认 state.json）极简，不值得独立 struct。`is_dir()` guard 与现有 `projects_dir` / `todos_dir` 一致。

### D2：`route_event` jobs 过滤策略——严格 `components.len() == 2 && file_name == "state.json"`

**选择**：path strip `jobs_dir` 前缀后，只认 `<job_id>/state.json`（两级 + 文件名匹配），其它路径（timeline.jsonl / pins.json / tmp/ / recap.trigger）全忽略。

**理由**：state.json 覆写频率分钟级，其它文件变更频率更高（timeline.jsonl 每秒写），过滤减少事件噪声。

### D3：`list_jobs` IPC 实现——全量扫描 + FileSignature cache

**选择**：`list_jobs` 每次被调扫 `~/.claude/jobs/*/state.json`，用 `FileSignature`（mtime + size）做 cache key。30 个 job × 2KB JSON ≈ 3ms 冷扫，cache hit 更快。

**替代**：维护内存 HashMap 由事件增量更新。

**理由**：job 数量少（通常 < 30），冷扫 < 5ms，增量维护引入一致性复杂度（crash 后 stale）。cache key 让二次调用几乎零成本。

### D4：分组逻辑——静态四组 + PR 优先

**选择**：
1. Ready for review：`children[]` 含 `kind:"pr"`
2. Needs input：`state === "blocked"`
3. Working：`state === "working"` 或 `state === "idle"`
4. Completed：`state in ["done","failed","stopped"]` 且无 open PR

**理由**：对齐 `claude agents` 原生 CLI 的分组语义。

### D5：Badge 优先级——红 > 黄 > 绿，working/空不显示

**选择**：Badge 只反映"需要你操作"：failed(红) > blocked(黄) > ready-for-review(绿)。working/idle/done/stopped 和空列表不显示 badge。

**理由**：badge 是注意力中断——只在用户需要介入时闪现。

### D6：Session 跳转—— `openSessionTab(sessionId, projectId)` + linkScanPath 截取

**选择**：从 `state.json.linkScanPath` 提取 `projects/` 后第一段作为 projectId（与 `encode_path(cwd)` 输出一致）。`linkScanPath` 为空时 fallback `encode_path(cwd)`。

**理由**：`openSessionTab` 已验证跨项目可行，不依赖当前 sidebar 选中的 project。

### D7：broadcast capacity——32

**选择**：`broadcast::channel::<JobChangeEvent>(32)`。

**理由**：job 数量少 + 事件频率低（分钟级），32 足够容纳短时突发；`Lagged` 静默跳过（前端下次收到全量刷新）。

### D-V1：选中态用 tonal lift 不用蓝

**选择**：选中行用 `background: var(--color-surface-raised)` + 左边 2px indicator（`var(--color-border-emphasis)`），不用 `--color-accent-blue`。

**理由**：The Persistent Selection Is Quiet Rule——蓝色保留给"正在进行"（working spinner），选中态用 tonal lift 不抢视觉。

### D-V2：Working 行 10px secondary spinner 不做降级

**选择**：working 状态的行级 indicator 用 10px CSS animation spinner（1.2s linear infinite rotate），不做"working 超 N 个则关 spinner"的降级。

**理由**：The One Live Signal Rule 允许 row-level secondary N 个并存（SubagentCard 先例）；job 数量有限（通常 < 10 working），不会产生视觉过载。

## Visual Contract

### Surface Decision

Jobs 面板作为 tab 级视图（PaneView 路由），入口在 TitleBar zone-status 区（UpdateStatusPill 与通知铃之间）。不做 sidebar 分区（job 跨项目）、dashboard 独占（有 tab 时看不到）、浮层 popover（信息密度不够）。

### Visual Layer

- **The Border Before Shadow Rule**：列表容器用 1px `var(--color-border-default)` border，不加 shadow
- **The Tool Density Rule**：行内 name 13px / detail 12px muted / metadata 10-11px mono
- **The Machine Information Rule**：branch、PR#、时间戳用 `var(--font-mono)`
- **The Status Owns the Color Rule**：只有状态 indicator dot/spinner 着色，行文本保持 neutral
- **The Ongoing Owns Blue Rule**：working = blue spinner
- **The Conflict Is Warning Not Error Rule**：blocked = amber dot
- **The Static-vs-Live Shape Rule**：working 用动态 spinner，其余状态用静态 dot
- **The Persistent Selection Is Quiet Rule**：选中行 tonal lift + left indicator，不用蓝

### State Coverage

| 状态 | 视觉 | 实现位置 |
|---|---|---|
| Working | 10px blue spinner + blue text | JobRow.svelte indicator |
| Blocked | amber dot | JobRow.svelte indicator |
| Idle | muted dot | JobRow.svelte indicator |
| Done | green dot | JobRow.svelte indicator |
| Failed | red dot | JobRow.svelte indicator |
| Stopped | grey dot (60% opacity) | JobRow.svelte indicator |
| 选中态 | tonal lift + left 2px | JobRow.svelte selected class |
| 展开态 | chevron 旋转 90° + raised surface | JobRow.svelte expanded |
| 空列表 | "No background jobs" grey text | JobsView.svelte empty state |
| 降级（目录不存在） | 入口隐藏 | UnifiedTitleBar conditional render |
| 降级（SSH 模式） | 入口隐藏 | UnifiedTitleBar conditional render |

### DESIGN.md delta plan

本次引入的可沉淀 token：
- Jobs 状态色映射（6 态 → CSS var）
- Badge 三色优先级系统（red > amber > green）
- Row-level secondary spinner pattern（10px / 1.2s linear）

archive 前跑 `/impeccable extract` 提进 `DESIGN.md`。

### D8：Job 删除机制——直接调 `claude rm`

**选择**：
- 单条删除：hover 行显示 × 按钮 → 第一次点击按钮变为红色 "确认?" → 第二次点击执行 `claude rm <short_id>`；3 秒无操作自动回退初始态
- 批量清理：Completed group header 显示 "Clear" 按钮 → 同样二次确认（"Clear" → "确认清除 N 项?"）→ 遍历 Completed group 逐条调 `claude rm`
- × 按钮仅对 terminal 状态 job 显示（Working/Blocked 是活跃状态，用 stop 而非 delete）
- inline 确认模式（不用 modal 弹窗）：点击态变化 + 超时回退，流程不中断

**替代**：
- (a) 直接 `rm -rf ~/.claude/jobs/<id>/` — 绕过 supervisor roster + worktree 清理，状态不一致
- (b) modal 确认弹窗 — 打断操作流，对频繁清理不友好
- (c) 无确认直接删 — 误触风险（用户反馈需要二次确认）

**理由**：`claude rm` 已封装好安全逻辑（清理 worktree 分支、从 supervisor roster 移除）。对 busy session 也能执行（CLI 实测确认）。CLI 输出的 worktree 保留提示可忽略（GUI 不展示 CLI stdout）。

**实测数据**：`claude rm 452c738b` → `removed 452c738b / worktree is on a different branch — kept at ...`。目录 `~/.claude/jobs/452c738b/` 已被删除。

### D9：Completed 组视觉层级——有 PR 的不淡化

**选择**：
- Completed + 有 PR → 正常文本色（opacity 1.0）+ PR chip 保持绿色高亮 → 用户仍能看到并点击 PR 链接
- Completed + 无 PR → 整行降低 opacity（0.55）→ 真正"完成且无后续动作"的 job 淡出视觉
- Failed → 保持红色 indicator 不淡化（需要用户注意）

**替代**：
- (a) 单独 "Ready for review" 分组 — 需要 GitHub API 验证 PR 状态（Plan B 已否决）
- (b) 全部 completed 统一灰色 — 用户反馈"不利于 review"

**理由**：用户核心诉求是"completed 不代表不需要操作"。有 PR 的 job 用户仍需去 review/merge，视觉不应暗示"忽略"。Opacity 而非 color 做淡化，让 PR chip 绿色在淡化行上仍可识别。

### D10：信息密度优化——收紧行间距

**选择**：行内 padding 从 10px 降到 7px，行间 gap 保持 0（分组内紧凑）。detail 行 line-height 从 1.4 降到 1.3。

**理由**：对标 CLI 的单行信息密度。GUI 本身有分组 header 带来的视觉分隔，行内不需要额外间距。

## Risks / Trade-offs

- **[Risk] state.json 格式变化** → Mitigation：serde 加 `#[serde(default)]` 容错 + unknown fields 忽略
- **[Risk] jobs 目录突然出现** → Mitigation：启动 stat + 切回 Jobs tab 时 re-check；不做定期轮询
- **[Risk] 大量 jobs（> 100）** → Mitigation：Phase 1 不分页，列表上限 ~50 个 active job 实测性能可接受（state.json 单个 < 5KB）
- **[Risk] linkScanPath 格式不稳定** → Mitigation：fallback 到 `encode_path(cwd)`，两者都失败则禁用跳转按钮
- **[Risk] `claude rm` 删除 busy session** → Mitigation：UI 只对 terminal 状态显示 × 按钮；批量清理仅覆盖 Completed 组；CLI 自身允许删 busy 是设计意图不是 bug
- **[Risk] 批量删除 N 个 job 串行调 CLI** → Mitigation：job 数量通常 < 10，每次 `claude rm` 耗时 < 200ms，串行 < 2s 可接受；未来可改 join_all 并发

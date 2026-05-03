## Context

紧接 change `sidebar-align-original` 的两条复盘修订：

1. **messageCount 算多了**：本仓 PR #38 落地后，用户对比原版仍发现 sidebar 消息数明显高于原版。Trace 到 `cdt-api/src/ipc/session_metadata.rs:64-79`：
   ```rust
   if msg.category == MessageCategory::User && !msg.is_meta {
       message_count += 1;
   }
   ```
   仅过滤 `category != User` 与 `is_meta`。本端口 cdt-parse 把 hard noise（local-command-stdout/stderr/caveat、system-reminder、empty-output）、interrupt、synthetic 都分类剥离了，但 **tool_result-only user 行**仍归 `MessageCategory::User`——`cdt-parse/src/noise.rs::extract_user_text` 在 `MessageContent::Blocks` 内只拼接 `ContentBlock::Text { text }`，无 text block 时返回 `None` 让 `classify_user_content` 短路返回 None，这条 user msg 既没被归 noise 也没拿到任何文本特征，成为漏网。原版 `isParsedUserChunkMessage`（jsonl.ts:165-226）的额外过滤："Blocks must contain text or image blocks for real user input"——本端口没有等价过滤。

2. **git 分支位置错位**：change `sidebar-align-original` 的 D3 决策"显示 active session gitBranch，无 active 回退 sessions[0]"——基于错误假设"branch 与 project 绑定"。实际 `gitBranch` 来自 JSONL 每行（每条 message），是 per-session（实为 per-message，本仓取最后一条非空）的属性。同 project 不同 session 可能在不同 branch（用户在 chat 中跑 `git checkout` 后开新 session）。原版 SidebarHeader Row 2 的 worktree 选择器表达"切 worktree 看不同 branch 的 sessions"，本端口不实现 worktree 切换；单栏 active-fallback 在用户切 session 时跟着变，与"反映项目所在分支"语义并不匹配——这是一个伪 per-project 视图。

## Goals / Non-Goals

**Goals:**
- messageCount 与原版数值对齐——同一 session 文件本仓与 TS 原版输出**相等**（误差 = 0）
- git 分支显示位置改到每条 SessionItem，per-session 语义清晰
- 不破坏既有 IPC 契约（`SessionSummary.gitBranch` 保留）；只改 UI 渲染位置与后端计数算法

**Non-Goals:**
- 不引入 worktree 切换 UX
- 不改 `gitBranch` 后端取值方向（保持"最后一条非空"——虽 codex 之前提示原版 `analyzeSessionFileMetadata:439` 取**第一条**非空，但本端口刚 archive 选定"最后一条"，未观察到用户明显抱怨；保留待后续 followup）
- 不引入新 IPC 字段；不改 Tauri command 列表

## Decisions

### D1: messageCount 算法在哪一层修？

**选择**：在 `cdt-api/src/ipc/session_metadata.rs` 加 `is_user_chunk_message` 过滤函数（与 cdt-parse / cdt-analyze 解耦）。

**候选**：
- (a) 在 session_metadata.rs 加专用过滤函数
- (b) 把 tool_result-only 的 user 消息在 `cdt-parse::noise` 里也归 hard noise（影响所有下游）
- (c) 把过滤函数提到 cdt-core 让 cdt-analyze 等其他消费者也能复用

**取舍**：
- (a) 最小改动，不影响 chunk-building / context-tracking 等其他下游（它们已通过自己的逻辑过滤；改 noise 分类可能引发回归）
- (b) 语义偏离——tool_result 不是"噪声"，chunk-building 仍需要它配对 tool_use；改 noise 分类会破坏现有 chunk 行为契约
- (c) 跨 crate 提升过早；只有 session_metadata 这一处需要"原版 isParsedUserChunkMessage 等价过滤"

实现签名：
```rust
fn is_user_chunk_message(msg: &ParsedMessage) -> bool {
    if msg.category != MessageCategory::User { return false; }
    if msg.is_meta { return false; }
    match &msg.content {
        MessageContent::Text(_) => true,  // hard noise / interrupt 已被 cdt-parse 分类剥离
        MessageContent::Blocks(blocks) => blocks.iter().any(|b|
            matches!(b, ContentBlock::Text { .. } | ContentBlock::Image { .. })
        ),
    }
}
```

### D2: SessionItem meta 行 branch 显示位置

**选择**：第二行末尾追加 `· <git-icon>{branch}` chip（与 messageCount / time 同行）。

**候选**：
- (a) 第二行末尾（messageCount · time · branch 三段）
- (b) 第三行新增（行高从 44px → 60px）
- (c) Hover 才显示 tooltip

**取舍**：
- (a) 行高 44px 保持，水平略密但 branch 名长用 ellipsis；与原版"信息密度高"一致
- (b) 列表行高加 36% → 同屏 sessions 数显著减少；需调整 ITEM_HEIGHT 常量与虚拟滚动 overscan
- (c) 用户要 hover 才能看到，不直观

承认 trade-off：水平拥挤场景（窄 sidebar + 长 branch）依赖 ellipsis；用户可拖宽 sidebar 缓解。

### D3: SidebarHeader 减少哪些 props？

**选择**：删除 `sessions`、`activeSessionId`；保留 `onToggleCollapsed`、`projects`、`selectedProjectId`、`onSelectProject`。

**理由**：移除 branch row 后 SidebarHeader 不再需要 session 数据；`Sidebar.svelte` 内部仍需要 `activeSessionId` 做高亮，所以 Sidebar 的 props 不变（活在 `App.svelte → Sidebar` 链路），只是不再向下透传给 SidebarHeader。

### D4: 已 archive 的 `项目 git 分支只读栏` Requirement 怎么办？

**选择**：`REMOVED` 该 Requirement，并在 `会话项展示` `MODIFIED` 块中加上 git 分支 chip 条款。

**候选**：
- (a) REMOVED 旧 + MODIFIED 既有"会话项展示"加 git 字段
- (b) 保留旧 Requirement 改为"per-session（在 SessionItem 行内）"——MODIFIED 该 Requirement
- (c) ADDED 新"会话项 git 分支显示"独立 Requirement，不动旧

**取舍**：
- (a) 语义清楚——旧位置不再渲染（彻底移除），新位置在"会话项展示"已有 Requirement 内自然扩展
- (b) Requirement 名"项目 git 分支只读栏"与新行为不符，强行 MODIFIED 名实不符
- (c) 旧 Requirement 留着但实际无对应 UI 渲染，spec 与代码脱节

按 OpenSpec 约定 `REMOVED Requirements` 必须含 `**Reason**` 与 `**Migration**`：
- Reason：分支信息位置错位（per-session 属性放在了项目级位置）
- Migration：信息已迁移到每条 SessionItem 第二行 meta 末尾

## Risks / Trade-offs

- **messageCount 数值大幅下降可能让用户误以为"活动减少"**：实际是修正——首次升级后用户会看到每个 session 的消息数明显减少（典型场景：含 50 次工具调用的 session 从 ~110 降到 ~10）。在 release notes 注解此为"修计数 bug"对齐原版即可。
- **fixture / 单测的 messageCount 期望值需要重新计算**：cdt-api 现有 metadata stream 测试 fixture 已经是无 tool_use 的简单 user/assistant 配对，不会受影响；但 `multi-project-rich.ts` fixture 中的 `messageCount` 字段是手填 mock 数据，无需修改（本 change 不动后端真实计算下游）。
- **SessionItem meta 行 branch chip 视觉拥挤**：窄 sidebar（200px） + 长 branch 名（如 `feature/refactor-very-long-name`）会触发 ellipsis；接受——用户拖宽 sidebar 即可。
- **e2e 测试需要更新**：`sidebar-collapse-and-branch.spec.ts` 第三个 test 改为断言 SessionItem 行内可见 branch 文本，之前的断言（`.branch-row .branch-name`）需要更新选择器。

## Migration Plan

无数据迁移。前端老 bundle 收到带 `gitBranch` 的 IPC payload 后只是渲染位置变化（branch 不再在 SidebarHeader，转到 SessionItem）；后端字段不变。

## Open Questions

无。

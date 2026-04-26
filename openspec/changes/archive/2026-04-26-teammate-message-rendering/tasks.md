## 1. cdt-core 类型扩展

- [x] 1.1 在 `cdt-core::chunk` 模块新增 `TeammateMessage` 结构体（字段：`uuid / teammate_id / color / summary / body / timestamp / reply_to_tool_use_id / token_count / is_noise / is_resend`），derive `Debug / Clone / Serialize / Deserialize / PartialEq`，serde camelCase
- [x] 1.2 在 `AIChunk` 结构体新增 `pub teammate_messages: Vec<TeammateMessage>` 字段（带 `#[serde(default, skip_serializing_if = "Vec::is_empty")]`），更新 `AIChunk::new()` / 任何构造点初始化为空 Vec
- [x] 1.3 `cargo clippy -p cdt-core --all-targets -- -D warnings` + `cargo test -p cdt-core`

## 2. cdt-analyze::team 检测函数

- [x] 2.1 新建 `cdt-analyze/src/team/noise.rs`，实现 `detect_noise(body: &str, teammate_id: &str) -> bool`（按 design D4 算法），覆盖 5 个单测场景（system idle JSON / system 短文本 / system 长文本不算 noise / 非 system idle JSON / 普通业务消息不算 noise）
- [x] 2.2 同文件实现 `detect_resend(summary: Option<&str>, body: &str) -> bool`（5 条原版正则），覆盖 4 个单测场景（summary 命中 / body 前 300 字符命中 / body 300 字符外不命中 / 全无关键词）
- [x] 2.3 在 `cdt-analyze/src/team/mod.rs` 导出新模块与 `pub use noise::{detect_noise, detect_resend};`
- [x] 2.4 `cargo test -p cdt-analyze --lib team::noise`

## 3. cdt-analyze::team 配对函数

- [x] 3.1 新建 `cdt-analyze/src/team/reply_link.rs`，实现纯函数 `link_teammate_to_send_message(teammate_id: &str, candidate_chunks: &[&AIChunk], used: &mut HashSet<String>) -> Option<String>`（按 design D2 算法：跨 turn 后向扫描，回溯上限 3 个 AIChunk，per-tool_use_id 去重）
- [x] 3.2 单测覆盖 8 场景：(a) 同 AIChunk 内配对 (b) 跨上一个 AIChunk 配对 (c) 多 teammate 同 SendMessage 第一条配对/第二条 orphan (d) 不同 recipient 跳过 (e) 回溯 4 个 AIChunk 失败 orphan (f) tool_executions 为空时 orphan (g) 已 used set 命中跳过 (h) tool input 不含 recipient 字段时跳过
- [x] 3.3 在 `cdt-analyze/src/team/mod.rs` 导出 `pub use reply_link::link_teammate_to_send_message;`
- [x] 3.4 `cargo test -p cdt-analyze --lib team::reply_link`

## 4. cdt-analyze::chunk::builder 状态机改造

- [x] 4.1 在 `cdt-analyze/src/chunk/builder.rs` 顶部加 `const EMBED_TEAMMATES: bool = true;`
- [x] 4.2 新增 `apply_teammate_embed(buffer_owner: &mut AIChunk, pending: &mut Vec<TeammateMessage>, prior_emit: &[Chunk], used_ids: &mut HashSet<String>)` 函数：在 AIChunk push 到 out 之前批量把 pending teammate 配对（调 `link_teammate_to_send_message`）后 move 进 `buffer_owner.teammate_messages`，清空 pending（实际落点：`flush_buffer` 内联实现 + `link_against_chunks` 私有 helper + `drain_trailing_teammates` 兜底）
- [x] 4.3 改写 `MessageCategory::User` 分支的 teammate 处理：`if EMBED_TEAMMATES && is_teammate_message(msg)` → 调 `parse_teammate_attrs` + `detect_noise` + `detect_resend` + `token_count` 估算（取 `msg.usage.as_ref().map(|u| u.input_tokens)` 或 fallback `body.chars().count() / 4`），构造 `TeammateMessage` push 到 pending；继续 `continue`（不 flush buffer）。`!EMBED_TEAMMATES` 时保留旧 `continue` 路径
- [x] 4.4 在 `flush_buffer` 内、AIChunk 构造完成但未 push 之前调 `apply_teammate_embed`；若主循环结束 pending 仍非空，把它们追加到 `out.last_mut()` 是 AIChunk 的那条；不存在 AIChunk 时静默丢弃
- [x] 4.5 单测覆盖 5 场景（参见 chunk-building/spec.md ADDED Requirement 的 Scenarios）：teammate 不产 UserChunk / 嵌入合并 AIChunk + reply_to 配对 / 末尾追加最后一个 AIChunk / 全 teammate 无 AIChunk 不 panic / 多队友各自配对自己的 SendMessage（`EMBED_TEAMMATES=false` 走 const 早绑定无法 runtime 测，文档化为常量切换时手测）
- [x] 4.6 `cargo clippy -p cdt-analyze --all-targets -- -D warnings` + `cargo test -p cdt-analyze`

## 5. cdt-analyze 既有快照接受

- [x] 5.1 跑 `INSTA_UPDATE=always cargo test -p cdt-analyze` 接受因 `AIChunk` 新增可选字段产生的快照变化（实际无 teammate 的 fixture 因 `skip_serializing_if` 无 diff，仅含 teammate fixture 才更新）
- [x] 5.2 `git diff -- crates/cdt-analyze/tests/snapshots/` 人工 review 每个变化是否符合预期；提交快照（无变更，跳过提交）

## 6. cdt-api session_metadata 标题修复

- [x] 6.1 在 `cdt-api/src/ipc/session_metadata.rs::sanitize_for_title` 追加 teammate-message 剥除循环（attributes 形式靠前缀匹配，与无 attribute 的 7 个标签并列）
- [x] 6.2 新增 `extract_teammate_summary_title` fast-path：trim 后 text 以 `<teammate-message` 开头时手解 attributes 抽 `summary="..."` 内容；非空时返回作为标题
- [x] 6.3 `extract_session_metadata` 在 `is_command_content` 与通用清洗之间插入 teammate fast-path 分支
- [x] 6.4 单测覆盖 5 场景：含 summary 取 summary / 无 summary 返 None / 非 teammate 返 None / 混合内容剥标签 / 无 attributes 边界
- [x] 6.5 `cargo clippy -p cdt-api --all-targets -- -D warnings` + `cargo test -p cdt-api --lib ipc::session_metadata`

## 7. cdt-api IPC 集成测试

- [x] 7.1 fixture 用 inline `ParsedMessage` 序列代替（更精确控制 chunk 边界；`get_session_detail` 路径硬编码 `~/.claude/projects/` 解析无法在 tempdir 测试）
- [x] 7.2 新建 `crates/cdt-api/tests/get_session_detail_with_teammate.rs`：直接调 `cdt_analyze::build_chunks` + `serde_json::to_value` 跑 IPC 边界序列化断言，覆盖 camelCase 字段集 + `replyToToolUseId` 配对（与 `get_session_detail` 经过的路径等价）
- [x] 7.3 新增 orphan 场景测试：teammate-message 在无 SendMessage 配对时 `replyToToolUseId` 字段 null/缺失；并增"无 teammate 嵌入时 IPC payload 不含 `teammateMessages` 键"+"多 teammate 各自配对"两个补充场景
- [x] 7.4 `cargo test -p cdt-api --test get_session_detail_with_teammate`（4 测全过）

## 8. UI 类型与组件

- [x] 8.1 在 `ui/src/lib/api.ts` 新增 `TeammateMessage` interface（10 字段 camelCase）；`AIChunk` interface 加 `teammateMessages?: TeammateMessage[]`
- [x] 8.2 `ui/src/lib/teamColors.ts` 已存在（先前 SubagentCard 已建），直接复用 `getTeamColorSet`
- [x] 8.3 在 `ui/src/lib/lazyMarkdown.svelte.ts` 的 `Kind` union 加 `"teammate"` 分支 + `estimatePlaceholderHeight` switch 同步加 case
- [x] 8.4 新建 `ui/src/components/TeammateMessageItem.svelte`：实现 noise / resend / normal 三态视觉契约；props 含 `teammateMessage` + `attachBody: AttachFn`（lazy markdown attach 工厂，由父级注入）+ 可选 `rootSessionId`；新增 `CORNER_DOWN_LEFT` / `REFRESH_CW` lucide icon 到 `ui/src/lib/icons.ts`

## 9. UI 渲染流接入

- [x] 9.1 修改 `ui/src/lib/displayItemBuilder.ts`：新增 `TeammateMessageDisplayItem` union 分支；首版按 `replyToToolUseId` 紧贴 SendMessage 插入。**13.6 修订**：改为按 `timestamp` 与所有 displayItems 整体稳定排序穿插（teammate 主动发言无 SendMessage 配对时也能正确穿插），`replyToToolUseId` 仅作 chip 文本不决定位置，对齐原版 `sortDisplayItemsChronologically`。
- [x] 9.2 在 `ui/src/routes/SessionDetail.svelte` 的 AIChunk 渲染流 switch 内新增 `{:else if item.type === "teammate_message"}` 分支，渲染 `<TeammateMessageItem teammateMessage={...} attachBody={attachMarkdown(body, "teammate")} rootSessionId={sessionId} />`；`attachMarkdown` 类型注解同步加 `"teammate"`
- [x] 9.3 `npm run check --prefix ui`（0 errors，5 个 warnings 是预先存在的 a11y / state_referenced_locally，与本 change 无关）
- [x] 9.4 `buildSummary` 对齐原版 `displaySummary.ts`：team 成员（`process.team` 非空的 subagent）按 unique `memberName` 计入 "N teammates" 而不是 "subagents"；`teammate_message` 单独计为 "N teammate messages"；拼接顺序 thinking → tool calls → messages → teammates → subagents → slashes → teammate messages

## 10. 端到端验证

- [x] 10.1 `cargo build --workspace` 整体编译过
- [x] 10.2 `cargo clippy --workspace --all-targets -- -D warnings` 严格通过
- [x] 10.3 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] 10.4 `cargo fmt --all`
- [x] 10.5 `cargo test --workspace`（全测过；新增 cdt-core 4 测 + cdt-analyze noise 6 + reply_link 10 + builder 5 + cdt-api session_metadata 5 + IPC 集成 4，共 34 个新测全过）
- [x] 10.6 `npm run check --prefix ui`（0 errors）
- [x] 10.7 `openspec validate teammate-message-rendering --strict`
- [ ] 10.8 `just dev` 启动桌面应用，打开一个真实的 team session，肉眼验收：sidebar 标题不再吐 XML / 主面板每条 SendMessage 紧贴对应 teammate 卡片 / noise 渲染单行 / resend 卡片半透明 + RefreshCw（**等用户验收**）

## 11. 验收并提交

- [ ] 11.1 等待用户验收（auto 模式：用户验收后才 push / 开 PR）
- [ ] 11.2 用户拍板后 `git add` 全部产出，commit message 含 `feat(teammate): ...` + `Co-Authored-By: Claude`
- [ ] 11.3 push 并开 PR

## 12. Archive

- [ ] 12.1 PR 内追加 archive commit：`/opsx:archive teammate-message-rendering` 把 5 份 spec delta（chunk-building / team-coordination-metadata / tool-execution-linking / ipc-data-api / session-display）同步合回主 spec
- [ ] 12.2 `openspec validate --all --strict` 通过
- [ ] 12.3 PR 合并后无后续动作（archive 已在 PR 内完成）

## 13. F-bug 修复轮（apply 阶段实测发现）

- [x] 13.1 `cdt-core::ToolExecution` 新增 `teammate_spawn: Option<TeammateSpawnInfo>` 字段；4 个 unit test
- [x] 13.2 `cdt-analyze::tool_linking::pair::extract_teammate_spawn` 在 pair 阶段从 user msg `toolUseResult.status == "teammate_spawned"` 抽 name/color 落到 ToolExecution
- [x] 13.3 `cdt-analyze::team::detection::parse_all_teammate_attrs` 多 block 解析（global regex），4 个新 unit test
- [x] 13.4 `cdt-analyze::chunk::builder::build_pending_teammates` 改返回 Vec，多 block 各自产 TeammateMessage，uuid 加 `-{idx}` 去重
- [x] 13.5 `ui/src/lib/api.ts` 加 `TeammateSpawnInfo` interface + `ToolExecution.teammateSpawn?` 字段
- [x] 13.6 `ui/src/lib/displayItemBuilder.ts`：teammate_message 改按 timestamp 排序穿插（不按 reply_to 紧贴 SendMessage）；teammate_spawn DisplayItem 新增并替换有 spawn 的 tool execution；buildSummary 加 teammate_spawn 计入 teammates
- [x] 13.7 `ui/src/routes/SessionDetail.svelte` switch 加 `teammate_spawn` 分支极简单行渲染 + import getTeamColorSet + CSS
- [x] 13.8 `design.md` 更正 D5 决策（按 timestamp 穿插 + reply_to 仅 chip）+ 新增 D5b（多 block / teammate_spawn）
- [x] 13.9 spec delta 同步：session-display Render 规则改 timestamp 排序；team-coordination-metadata 新增"Parse all teammate-message blocks"；新增 tool-execution-linking spec delta"Detect teammate-spawned tool results"；ipc-data-api 加"Expose teammate spawn metadata"
- [x] 13.10 workspace clippy + src-tauri clippy + cargo test --workspace + npm check --prefix ui 全过；`openspec validate teammate-message-rendering --strict` 通过
- [ ] 13.11 用户肉眼验收：(a) 3 人团队 spawn 显示 3 个 "Teammate spawned" 极简单行 (b) member-2 / member-3 reply 各自独立卡片不再合并 (c) idle JSON 各自识别为 noise 渲染单行 (d) teammate / output / tool 按 timestamp 时序穿插，不再堆在 turn 末尾

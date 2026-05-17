## Why

会话列表 sidebar 显示的 title 时常与"用户记忆里这条会话的开场白"对不上：

1. **带参 slash 命令被降级到 fallback**：截图 case `sessionId=cecc12ae-ac6f-4164-b79b-5b99595b43c5` 的第一条 user 消息是 `/impeccable 根据项目的已有代码生成一下设计规范`（带 args 的功能性 slash），列表却显示第二条非命令消息「提一下PR吧，我审查一下」。当前 `extract_session_metadata_with_ongoing` 行 203-206 把所有 `<command-name>` 起首消息都进 `command_fallback` —— 用户的真实意图（写在 args 里）被覆盖。
2. **`[Request interrupted by user…` 当 title 显示**：用户上一轮按 ESC 触发的中断标记被 `sanitize_for_title` 漏过，字面量 `[Request interrupted by user during tooling cycle]` 直接进 title。
3. **`Read the output file to retrieve the result: /tmp/xxx` 系统指令残留**：`<task-notification>` 剥除后紧跟的 task 输出读取指令文本残留 title。
4. **Sidebar / Tab label 比详情页短**：后端 `truncate_str(&summary, 200)` 截 200 字符 + 前端 `tabStore::shortLabel` 又 JS 截 50 字符 + "…"（hover tooltip 也是截断版）—— 用户拉宽 sidebar / hover 都看不到完整 title。

## What Changes

- **MODIFIED** `cdt-api::session_metadata::extract_session_metadata_with_ongoing` title 提取规则：
  - 带 `<command-args>` 非空内容的 slash 命令 SHALL 直接作为 title（如 `/impeccable 根据项目的已有代码生成一下设计规范`），不再降级到 `command_fallback`
  - 无 args 的纯辅助 slash（`/clear` `/help` `/cost` 等）仍进 `command_fallback`，被后续 user 消息覆盖
  - 标题候选 trim 后以 `[Request interrupted by user` 起首时 SHALL 跳过该消息，继续找下一条
- **MODIFIED** `cdt-api::session_metadata::sanitize_for_title`：尾部追加正则 `/ ?Read the output file to retrieve the result: \S+/g` 移除 task-notification 后的系统指令残留
- **MODIFIED** title 截断长度：`truncate_str` 调用统一抽取常量 `TITLE_MAX_CHARS = 500`（teammate summary / 普通 sanitize / 新的 slash-with-args 三处）—— 拉宽 sidebar / hover tooltip 显示完整标题的空间
- **MODIFIED** 前端 `tabStore::shortLabel` 删除 50 字符 JS 截断；TabBar 标签改为纯 CSS ellipsis + `max-width` 视觉截断，HTML `title` 属性传 full title 让 hover tooltip 显示完整字符串

无 IPC 字段 / Tauri command 协议改动。`EXPECTED_TAURI_COMMANDS`、`LocalDataApi` 公开方法签名、`SessionSummary` struct 字段集均不变。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `ipc-data-api`：`extract_session_metadata` title 提取规则修订（slash 带 args 作 title / 跳过 interrupted 消息 / sanitize 加 task-output 指令清洗 / truncate 长度 500）
- `sidebar-navigation`：Tab label 截断策略 SHALL 由前端 CSS ellipsis + `max-width` 实现，JS 侧 SHALL NOT 再做不可逆字符截断；HTML `title` 属性 SHALL 提供完整字符串供 hover tooltip 显示

## Impact

**代码**：
- `crates/cdt-api/src/ipc/session_metadata.rs`：4 处算法调整 + 常量提取
- `crates/cdt-api/tests/ipc_contract.rs`：可能加 SessionSummary.title 字段长度上限的 round-trip 测试
- `ui/src/lib/tabStore.svelte.ts`：删 `shortLabel` 截断
- `ui/src/components/TabBar.svelte`：tab label 加 `max-width` + 确认 ellipsis + tooltip 字段
- `ui/src/lib/__fixtures__/*.ts`：必要时补一条 long-title fixture 验证 hover 行为

**spec**：
- `openspec/specs/ipc-data-api/spec.md`：modified 一个已有 Requirement（title 提取规则）或新加 Requirement `Sanitize title against interruption and task-output instructions`
- `openspec/specs/sidebar-navigation/spec.md`：modified Tab 标签截断 Scenario / 新加 hover tooltip Scenario

**性能**：纯字符串处理 / 算法调整，不增加 I/O、不增加 IPC payload bytes。MetadataCache 200 字符 → 500 字符增量 RAM ≈ 200 entries × 300 chars × 2 ≈ 120 KB，可忽略。

**用户可见**：
- 列表 title 更贴近"用户记忆里第一句话"
- 长 title 在拉宽 sidebar 后可显示更多，hover tooltip 总能显示 full title
- 已存在缓存条目命中时仍是旧 title —— `FileSignature` 不变缓存不失效（不刻意 invalidate，节省启动 IO；新逻辑只对新扫描 / 缓存更新生效，自然过渡）

**风险**：
- slash 带 args 作 title 改变了用户可见行为，与 TS 原版语义不同；但用户视角"第一句话"语义更直观，且 design.md D1 有显式取舍记录
- 缓存中旧 title 不主动 invalidate，部分会话短期内还是旧 title —— 重启 / 文件改动后自然刷新；可接受

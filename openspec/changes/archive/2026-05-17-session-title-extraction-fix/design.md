## Context

会话标题提取链路：

```
JSONL → cdt-parse → ParsedMessage → cdt-api::session_metadata::extract_session_metadata_with_ongoing
                                       ↓ (前 200 行 + title.is_none() gate)
                                       extract_text → is_command_content?
                                                   → is_command_output? → skip
                                                   → extract_teammate_summary_title?
                                                   → sanitize_for_title
                                       ↓
                                       truncate_str(200)
                                       ↓
                                       SessionSummary.title
                                       ↓ (Tauri IPC → camelCase)
                                       ui Sidebar (CSS ellipsis, HTML title 全文)
                                       ui TabBar  (shortLabel JS 截 50 + "…")
```

**当前痛点**（4 处独立，互不耦合）：
1. `is_command_content` 命中的消息一律进 `command_fallback`，被后续非命令消息覆盖 → 用户带参 slash 的真实意图丢失
2. `[Request interrupted by user…` 起首消息没被 sanitize 过滤 → 字面量当 title
3. `Read the output file to retrieve the result: /tmp/...` 残留 → task-notification 后系统指令进 title
4. 后端 200 字符 + 前端 50 字符 JS 截断双重不可逆截断 → 用户拉宽 sidebar / hover 仍看不到完整 title

**约束**：
- 缓存 `MetadataCache`（LRU 200）按 `FileSignature` 命中；不能因这次行为契约改动强行 invalidate 全量（启动 IO 不可接受）
- 不改 IPC 字段集 / Tauri command 协议
- 性能不能回退：标题提取在 metadata scan 内单次执行，新加的过滤 / 正则 / 常量化都是 O(L) L=单条消息长度，无新增 I/O
- 跨平台 / 跨缓存版本兼容：旧缓存条目的 title 是按旧规则算的，命中时直接返回旧值；新写入按新规则；不强制 invalidate

## Goals / Non-Goals

**Goals:**

- title 优先反映"用户视角的第一句对话内容"（带 args slash 的 args 部分纳入）
- title 不包含已知系统注入文本（interrupted 标记 / task-output 指令 / 任何 system tag）
- 标题截断长度与 hover tooltip 之间无信息丢失 —— 任意位置的用户都能完整看到 title 全文（hover / 拉宽 sidebar / Tab tooltip）
- 性能持平或略好（不增 I/O，不增 IPC payload bytes，纯算法改动）

**Non-Goals:**

- **不**做 title 缓存强制 invalidate —— 已有缓存条目按旧规则计算的 title 仍然返回，避免一次启动突发扫描；用户重启或文件改动后自然刷新
- **不**改 IPC 字段集（不加 `titleFull` / `titleDisplay` 等冗余字段；后端只输出一个 title 字段，前端按宽度截断显示）
- **不**做 i18n / RTL 截断特殊处理（CSS ellipsis 浏览器原生支持已足够）
- **不**做 markdown / code-fence 等富文本截断（title 是纯文本字符串）

## Decisions

### D1：slash 命令处理 —— 带 args 时直接作 title，无 args 进 fallback

**选择**：`<command-args>` 内容非空时（trim 后非空白），`extract_command_display` 拼成 `/name args` 直接赋值 title，不进 `command_fallback`。空 args / 无 `<command-args>` tag 时按当前逻辑进 fallback。

**理由**：
- 用户记忆锚点：用户写 `/impeccable 生成设计规范`时，"生成设计规范"是他记得的事；slash 名只是路由
- 区分功能性 slash（带 args，参数即 prompt）vs 辅助 slash（无 args，纯命令如 `/clear` `/help`）
- 辅助 slash 大多与会话主题无关，作 fallback 让真实对话内容覆盖更合理

**候选方案**：
- A1：所有 slash 一律作 title —— 拒，`/clear` 会盖掉真实主题
- A2：所有 slash 一律作 fallback（现状 = TS 原版） —— 拒，丢失带参 slash 的用户意图
- A3：带 args 作 title，无 args 作 fallback —— ✓ 选择
- A4：白名单 slash（`/impeccable` 等用户 slash 类入 title） —— 拒，白名单维护成本 / 用户自定义 slash 不可枚举

**取舍**：与 TS 原版语义分歧 —— TS `extractCommandName` 只输出 `/impeccable` 丢弃 args；本改动输出 `/impeccable 生成设计规范` 并直接作 title。CLAUDE.md 已说"按 spec 走不复刻 TS bug"，且用户明确说"原版不一定对，从用户视角"——具备背书。

### D2：过滤 `[Request interrupted by user` 起首消息

**选择**：在 `extract_text` 返回非空后、`is_command_output` 检查同位置（行 201 附近），加判断 `text.trim_start().starts_with("[Request interrupted by user")` → 跳过整条 user 消息（不进 title 也不进 fallback）。

**理由**：
- 该字面量由 Claude Code CLI 在用户按 ESC 时注入到 JSONL，是系统标记不是用户输入
- 字面量进 title 既不可读也无识别价值
- 与 TS 原版 `extractPreviewFromUserEntry` 行 149 + 182 行为对齐（这处 TS 行为是合理的）

**位置**：放在 `is_command_output` 之后、`is_command_content` 之前。这样 interrupted 文本既不被当命令输出（保持原 messageCount 计数行为），也不污染 `command_fallback`。

### D3：sanitize_for_title 加 `Read the output file to retrieve the result: <path>` 移除

**选择**：在 `sanitize_for_title` 现有 8 个 tag + teammate-message 移除循环之后，加一次正则 `re.replace_all(/ ?Read the output file to retrieve the result: \S+/g, "")`。**触发条件 SHALL** 限定为"原文确实含 `<task-notification>` 字面 tag"——剥 tag 前用 `text.contains("<task-notification>")` 提前判定（剥完后判断会失败）。

**实现**：用 `std::sync::OnceLock<Regex>` 编译一次复用（stdlib，无需引入 `once_cell` crate）。

**理由**：
- `<task-notification>` 标签被 sanitize 剥后，其前置 " Read the output file..." 文本残留
- TS 原版 `contentSanitizer.ts` 第 30 + 122 行 `TASK_OUTPUT_INSTRUCTION_PATTERN` 是**无条件 replace**，对用户在普通消息中手写同字面量（如教程引用）也会吞——本仓不复刻 TS 这点（codex apply 阶段二审反馈）
- 仅作用于 title 提取路径（`sanitize_for_title`），不影响其他位置的内容展示

### D4：title 截断常量化 `TITLE_MAX_CHARS = 500`

**选择**：抽取常量 `const TITLE_MAX_CHARS: usize = 500`，替换 `truncate_str(&summary, 200)` 三处调用。

**理由**：
- 当前 200 字符在中文环境下 ≈ 200 ~ 400 px 显示宽度，sidebar 拉宽到 500 px 也常常截断到 200 字符 + "…" —— 用户感知"拉宽也展示不出来"
- 500 字符对齐 TS 原版上限，留够 hover tooltip 显示空间
- 500 vs 200 内存增量 ≈ 200 cache entries × 300 chars × 2 bytes = 120 KB，可忽略
- IPC 单条 SessionSummary payload 增量上限 ~600 bytes，500 条 session 列表增量 ~300 KB，远小于 1 MB IPC 瘦身阈值

**候选方案**：
- B1：保持 200（与 TS 不对齐） —— 拒，用户痛点未解决
- B2：500（对齐 TS） —— ✓ 选择
- B3：1000+ —— 拒，超出 hover tooltip 可读性，无收益

### D5：前端 Tab 截断改纯 CSS，shortLabel 删除

**选择**：
- `ui/src/lib/tabStore.svelte.ts::shortLabel` 函数删除（或改为透传 `(label) => label`），所有调用点直接传 full label
- `ui/src/components/TabBar.svelte` 的 `.tab-label` CSS 加（或确认已有）`max-width` + `overflow: hidden` + `text-overflow: ellipsis` + `white-space: nowrap` 做视觉截断
- TabBar 的 `title={tab.label}` 属性自动获得 full title（因为 store 不再截断）

**理由**：
- JS 截断不可逆，hover tooltip 也只能看到截断版 —— 信息丢失
- CSS 截断响应宽度，配合 max-width 提供可控视觉宽度
- HTML `title` 属性原生提供 hover tooltip，无需额外组件
- 与 Sidebar 项的 title 处理对称（Sidebar 早已是纯 CSS 截断 + HTML title 全文，见 sidebar-navigation spec §"会话项展示"）

**候选方案**：
- C1：保留 50 字 JS 截断 + 增加 `tab.tooltip` 字段存 full title 用于 hover —— 拒，复杂度增加但不必要
- C2：JS 截断 100 字符兼容 —— 拒，问题本质不在长度而在 hover 信息丢失
- C3：纯 CSS + max-width，删 JS 截断 —— ✓ 选择

### D6：缓存兼容策略 —— 不强制 invalidate

**选择**：保留 `MetadataCache` 按 `FileSignature` 命中机制；不因这次行为契约改动 bump cache version 或强制 invalidate。

**理由**：
- 强制 invalidate 会导致下次启动重扫所有 session 文件（可能 500+），违反"性能不能回退"目标
- 旧缓存条目按旧规则计算的 title 在用户视角上"虽然不对，但不离谱"（要么是非命令第二条消息，要么是空），可接受短期遗留
- 文件改动 / 应用重启会自然过期；典型用户日活 1-2 个项目，受影响 session 数有限
- 风险：用户重装升级后部分 session title 看上去"没更新"—— 文档化为 known acceptable behavior

**候选方案**：
- E1：bump `MetadataCache` version 字段强制全 invalidate —— 拒，下次启动 IO 风暴
- E2：lazy invalidate（命中时同时按新规则重算并比较，不同则更新） —— 拒，等于每次命中重扫，cache 失去价值
- E3：不主动 invalidate —— ✓ 选择

**spec 兜底**：D6 在 codex 二审中被指出"只在 design.md 记录、未进 spec"；修法是在 `specs/ipc-data-api/spec.md` 加 ADDED Requirement `Title algorithm changes do not invalidate MetadataCache` + 2 个 Scenario（hit 返回旧 title / signature 变化后用新算法），让未来 reviewer / 实现者通过规范看到此约束。

### D7：边界单测补全（codex 二审追加）

codex design 阶段二审指出 tasks 2.1-2.8 漏 3 个边界：

- `<command-args/>` 自闭合：`extract_tag_content` 行 461-471 只识别 `<tag>...</tag>`，自闭合走"无 args"路径，需断言 fallback 行为
- sanitize 后只剩空白：`sanitize_for_title` 行 521 `trim()` 后可能为空，需断言走 fallback 路径
- `title.is_none()` early-exit gate 正向验证：第一条合法 title 后第二条不应覆盖

修法是 tasks 2.9 / 2.10 / 2.11 三条新增单测 + tasks 2b.1 / 2b.2 两条缓存兼容性单测。

## Risks / Trade-offs

- **[与 TS 原版语义分歧]** D1 的"slash 带 args 作 title"与 TS 原版不一致 → 用户从原版迁移过来可能感知到 title 变化 —— Mitigation：proposal 与 design.md 显式记录分歧；CLAUDE.md "按 spec 走不复刻 TS bug" 已背书；用户明确认可"原版不一定对"
- **[缓存遗留旧 title]** D6 不主动 invalidate → 部分老 session 短期内 title 仍按旧规则显示 —— Mitigation：文件改动 / 重启自然刷新；用户感知有限
- **[长 title 拖累 IPC payload]** 500 字符 vs 200 字符在 500 session 列表场景增量 ~300 KB —— Mitigation：远小于 1 MB IPC 瘦身阈值；前端按宽度截断显示无渲染压力
- **[regex 编译开销]** D3 引入一次 regex 编译 —— Mitigation：`once_cell::sync::Lazy<Regex>` 进程级一次编译
- **[interrupted 过滤误伤]** D2 跳过 `[Request interrupted by user` 起首的消息，理论上若用户自己输入这串字面量作为 prompt 会被误判 —— Mitigation：该字面量极不可能是用户主诉（重叠概率近 0），文档化 known behavior

## Migration Plan

- 新代码合并后，新建会话 / 文件改动后的会话自动按新规则计算 title
- 旧缓存条目按旧规则保留，自然淘汰
- 无需 DB / 配置迁移；无需用户操作

## Open Questions

- 是否在 sidebar-navigation spec 中显式约束 `tab.label === SessionSummary.title`（即 Tab label 永远来自后端 title，无前端再加工）？倾向 yes，落 spec delta 时统一约束。

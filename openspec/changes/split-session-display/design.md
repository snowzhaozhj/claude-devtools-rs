## Context

issue #303 9-PR plan 阶段 3 第二个 PR。前序 PR 5（change `cleanup-sidebar-navigation`，PR #322）+ PR 6（change `cleanup-scenario-naming`，PR #325 + #327）已 merge；PR 6 主动**排除** session-display / sidebar-navigation / ipc-data-api 三个待拆 cap，留给本 PR + PR 8 + PR 9 各自拆分时一起清。

session-display 当前规模：1390 行 / 45 Requirement / **219 Scenario**（issue #330 body 写 264 是误记，按实际 grep 计数为准）。TOP 3 体积仅次于 ipc-data-api。

按 issue #303::D-7 决策拆为 4 个 cap：

| cap | 性质 | Requirement | Scenario |
|---|---|---|---|
| `session-display`（保留主体） | 对话流编排骨架 + AI/User chunk 渲染 + Subagent 卡片 + Context Panel + 顶 bar + 锚点导航 + 多 pane | **33** | **162** |
| `markdown`（新建） | Markdown 渲染 + 代码高亮 + Mermaid + 无语言代码块自动检测限制 + Lazy markdown 渲染 | **4** | **18** |
| `tool-viewer-routing`（新建） | 工具专化查看器路由 + Tool row 展示 + Tool result 展开/折叠性能 + Tool detail timing/failure + Lazy load tool output + 大文本工具详情交互 | **6** | **31** |
| `edit-diff-view`（新建） | Edit 工具 Diff 视图 + Edit diff preview highlighting | **2** | **8** |
| **加和** | — | **45** | **219** |

issue #303 / #330 body 给的 `31+4+6+2 = 43` 是误记（应为 `33+4+6+2 = 45`）；本 design 以实际 spec 内容为真相。

工艺直接复用：

- change `cleanup-sidebar-navigation`（PR #322）—— 单 cap 14 Requirement 重写工艺
- change `cleanup-scenario-naming`（PR #325/#327）—— 跨 9 cap Scenario 标题改名 + D-3 改名表 + D-4 微妙边界保留清单

相比 PR #322 / #325 体量更大（12 Requirement + ~57 Scenario 跨 4 cap 移动 vs PR 322 单 cap 14 Req 重写 / PR 325 21 Req 跨 9 cap 标题改名），但工艺更"机械"（不重写、纯字符级迁移）。

## Goals / Non-Goals

**Goals:**

- `session-display` 主 spec 体量从 1390 行降到 ~1000 行（剥离 4+6+2=12 个 Requirement）
- 4 个 cap Scenario 总数与原始 219 严格相等；行为契约 100% 不变（**字符级**对等，不允许在迁移过程中修订 SHALL / WHEN / THEN 子句）
- 新 cap 的 `Purpose` 段从用户视角写（"提供什么能力"），不抄实现概要
- 顺手清理 SubagentCard ongoing Requirement 内 2 个 Scenario 标题违反 SPEC_GUIDE 反例 1 的内部 jargon（`（C1 修复）` `（C3 修复）` codex 审批后缀）
- 跨 spec 描述性引用 `tool-execution-linking::Source tool output text from raw tool_result.content` body 内 "session-display capability 的 ReadToolViewer" 改为 "tool-viewer-routing capability 的 ReadToolViewer"

**Non-Goals:**

- 不改代码 / 测试 / 配置 / IPC 字段名 / Tauri command 名（纯 spec 文档拆分）
- 不动 session-display 留下的 33 个 Requirement 内的 30+ 处主 spec body 历史污染（src 路径 / Rust 类型 / 内部 fn 名）—— 留后续单 cap 重组 PR / cleanup PR
- 不动其他 cap 对 session-display 的引用（agent-configs body 内一处描述性引用保持不变 —— subagent 彩色标识体系仍归 session-display）
- 不批量重写 12 个被 REMOVED Requirement 内的 body 历史污染 —— 它们随 REMOVED 整体迁移到新 cap，body 由新 cap owner 负责后续清理（spec-purity ratchet 在新 cap 注册 baseline，不增、不减）
- 不动 Requirement 标题（标题级 RENAMED 工艺成本相对收益不划算）—— Requirement 标题字符级搬运
- 不引入 BREAKING change：消费方（前端 Svelte 组件、IPC 消费者）不需修改任何代码；新旧 cap 名是 spec 内部组织维度，不影响运行时

## Decisions

### D-1：行为契约 100% 不变（字符级对等）

**问题**：12 个 Requirement 跨 cap 移动 + 1 个 Requirement MODIFIED Scenario 标题，若不小心修订了 SHALL / WHEN / THEN 子句，会破坏行为契约。

**决策**：每个 ADDED Requirement 的 body 与所有 Scenario WHEN/THEN/AND 子句**字符级**等于原 session-display spec.md 对应段落（仅去除 codex 审批后缀的 1 个 MODIFIED 例外，由 D-5 单独管控）。校验手段：`scripts/check-spec-purity.sh` baseline + 人工 diff 抽样。

**反例**（不允许）：把 `MUST` 改 `SHALL` / 把 `SHALL NOT` 改 `不得` / 拆开长句 / 顺手补 i18n 链接。这些"看起来更整洁"的微调本 PR 一律拒绝，留后续 cleanup PR。

### D-2：spec delta 工艺 — REMOVED + ADDED 而非 MODIFIED

**问题**：12 个 Requirement 离开 session-display，可以选两种 delta 写法：

1. **MODIFIED**：在 session-display delta 里把整个 spec.md 重写，标记 12 个 Requirement 为 `MODIFIED Requirement`（实际删除）
2. **REMOVED + ADDED**：session-display delta 内 `## REMOVED Requirements` 段列 12 个 Requirement 标题；新 cap 在 `## ADDED Requirements` 段写完整 body

**决策**：选 (2)。理由：

- **delta 体量**：(1) 让 session-display delta ≈ 1390 行（整个主 spec 重写），reviewer 字符级对照成本陡升；(2) session-display delta ≈ 50 行（仅列被 REMOVED Requirement 标题）
- **语义正确性**：被移走的 Requirement 不是被"修改成更新版本"，而是"从本 cap 消失，到另一个 cap 出现"。`REMOVED` 语义就是这个；`MODIFIED` 误暗示"在 session-display 内继续存在但内容变了"
- **历史可读性**：archive 后看 history 时，"PR 7 移走了 12 个 Requirement"用 `REMOVED` 表达直观；用 `MODIFIED` 需要 reviewer 自行 diff 才能看出"内容被搬走了"

**implementation**：

- `specs/session-display/spec.md` 仅含 `## REMOVED Requirements` 段（12 项标题列表）+ `## MODIFIED Requirements` 段（1 项 SubagentCard ongoing，由 D-5 单独写）。**不**含 `## ADDED Requirements`
- `specs/markdown/spec.md` / `specs/tool-viewer-routing/spec.md` / `specs/edit-diff-view/spec.md` 各自含 `## ADDED Requirements` 段，body 字符级搬运自原 session-display
- `openspec validate split-session-display --strict` 仍会校验：被 REMOVED 的 Requirement 在主 spec 中存在；被 ADDED 的 Requirement 在主 spec 中**不**存在 —— archive 时 `openspec archive` 自动 mv body 到新 cap 主 spec

### D-3：边界灰区裁定表

部分 Requirement 在内容上跨 ≥ 2 个新 cap 的语义边界。本 PR 显式裁定每条灰区的 owner cap 与理由。所有跨 cap 引用通过描述性文字（不是 Requirement 标题精确引用）维护，避免 cap 重命名时雪崩。

| # | Requirement 标题 | 候选 cap | **裁定 owner** | 理由 |
|---|---|---|---|---|
| 1 | `大文本工具详情交互优先渲染` | tool-viewer-routing / markdown / edit-diff-view | **tool-viewer-routing** | Requirement body 内 `Read` / `Write` / `Edit` 三工具展开时机判定混合一处；本质是"按 viewer 路由决定 IPC 拉取与展开节奏"，归 tool-viewer-routing；Edit diff 行不做高亮的 Scenario 与 markdown 高亮策略相关，但仍是 viewer 内部决策 |
| 2 | `Lazy markdown rendering for first paint performance` | markdown / session-display | **markdown** | "把所有 markdown 内容渲染延迟到入视口"是 markdown capability 的实现策略；session-display 通过描述性引用 `[[markdown]]` 调用 `flushAll()` 时机，不需精确引用 |
| 3 | `无语言代码块高亮自动检测限制` | markdown / tool-viewer-routing | **markdown** | "未声明语言不调 highlightAuto"是 markdown 渲染管线内决策，不限于工具 viewer 场景；user prose / AI lastOutput / Thinking body 的代码块都受同规则约束 |
| 4 | `Edit diff preview highlighting` | edit-diff-view / markdown | **edit-diff-view** | "diff 行按 file_path 推断语言高亮"是 EditToolViewer 专属渲染规则，与通用 markdown 代码块路径独立；degrade 到 plain 时也不走 markdown 管线 |
| 5 | `工具专化查看器路由` | tool-viewer-routing | tool-viewer-routing | 无歧义 —— 路由表本身就是 tool-viewer-routing 的核心 Requirement |
| 6 | `Tool row displays approximate token count` | tool-viewer-routing | tool-viewer-routing | row 渲染规则归 tool viewer |
| 7 | `Lazy load tool output on expand` | tool-viewer-routing | tool-viewer-routing | 展开时拉取 IPC 是 viewer 行为 |
| 8 | `Tool detail timing and failure visibility` | tool-viewer-routing | tool-viewer-routing | 工具明细 metadata 展示，归 viewer |
| 9 | `Tool result expansion avoids eager heavy rendering` | tool-viewer-routing / markdown | **tool-viewer-routing** | "折叠态不渲染 markdown / 高亮 / JSON DOM" 的判定主语是工具 row，由 viewer 路由决定；markdown 渲染管线只在被工具 viewer 调用时执行 |
| 10 | `Edit 工具 Diff 视图` | edit-diff-view | edit-diff-view | LCS diff 渲染是 EditToolViewer 专属 |
| 11 | `Markdown 渲染与代码高亮` | markdown | markdown | 通用 markdown 渲染管线 |
| 12 | `Mermaid 图表渲染` | markdown | markdown | mermaid 是 markdown 内嵌代码块的特殊渲染分支 |

**裁定原则**：

- 优先按 **DOM 渲染主体** 判 owner —— 谁渲染那个 DOM 节点，行为契约归谁
- 其次按 **触发时机** 判 owner —— 谁决定何时渲染（viewer 路由 / markdown 占位 observer），归谁
- 最后按 **数据消费形态** 判 —— `exec.input` 消费 vs `exec.output` 消费的策略归 viewer

### D-4：跨 spec 引用更新

**变化**：tool-execution-linking spec.md L54（`Source tool output text from raw tool_result.content` Requirement body 内）原文：

> UI 渲染层（`session-display` capability 的 `ReadToolViewer`）按需 strip 前缀

拆后 ReadToolViewer 归 tool-viewer-routing capability。该 body 改为：

> UI 渲染层（`tool-viewer-routing` capability 的 `ReadToolViewer`）按需 strip 前缀

**实施方式**：在 `specs/tool-execution-linking/spec.md` delta 内用 `## MODIFIED Requirements` 段重写整个 `Source tool output text from raw tool_result.content` Requirement body，仅替换该一处引用文字。Scenario 全 4 项字符级保持。

**保留不动**：

- `agent-configs/spec.md` Purpose 段内 "UI 层消费以实现 subagent 卡片的彩色标识体系（参见 `session-display`）" —— `Subagent 彩色标识体系` Requirement 留在 session-display，引用正确；不需更新

### D-5：SubagentCard ongoing Scenario 标题清理

**问题**：`SubagentCard 在 ongoing 期间主动重拉 trace` Requirement 内 2 个 Scenario 标题包含 codex 审批后缀：

- `首次展开期间版本跳变由 effect 接管（C1 修复）`
- `IPC 失败后折叠重开能重试（C3 修复）`

`（C1 修复）` / `（C3 修复）` 是 codex 二审 round 编号 + 审批意见的缩写，违反 `SPEC_GUIDE.md::反例 1` "Scenario 标题不写 codex round 编号 / PR 编号 / commit hash" 与 reviewer checklist 末两条 "Scenario 标题去除审批过程注释 / 内部 jargon"。

**决策**：本 PR 顺手清理这 2 个标题（PR 6 follow-up，因 PR 6 主动跳过 session-display）。改为：

- `首次展开期间版本跳变由 effect 接管`
- `IPC 失败后折叠重开能重试`

Scenario body / WHEN / THEN / AND 子句全部字符级保持（包括 body 内 "（codex 二审 C1 发现）" 字样保留 —— 这在 Requirement body 散文中作为历史注脚 OK，issue #303::D-7 reviewer checklist 仅约束 Scenario 标题层）。

**实施方式**：`specs/session-display/spec.md` delta 内 `## MODIFIED Requirements` 段重写整个 `SubagentCard 在 ongoing 期间主动重拉 trace` Requirement body（含全部 8 个 Scenario），仅 2 个 Scenario 标题去后缀，其余字符级保持。

**为什么不顺手清剩下 4 处 Scenario 标题命名问题**：扫描 session-display 留下的 33 个 Requirement，发现还有 ~5-7 处疑似 SPEC_GUIDE 反例 1 命中（含 `expandedChunks` / `tabId` / `displayItemBuilder` / `[[push-events::file-change]]` 等内部 fn / 类型 / spec 索引引用作为 Scenario 标题或子句）。本 PR 严格 scope 在"跟 REMOVED 同 commit 顺手清"原则下不扩散 —— 留后续单 cap cleanup PR 处理（与 PR #319 / #322 / #325 工艺一致）。

### D-6：新 cap Purpose 段的迁移路径

**问题**：openspec change delta schema 仅接受 `## ADDED / MODIFIED / REMOVED / RENAMED Requirements` 顶级段，**不**支持 `# X Specification` / `## Purpose` / `## Requirements` 这种主 spec 完整结构。`openspec validate split-session-display --strict` 会拒绝在 delta 内放 `## Purpose` —— 报 `No delta sections found. Add headers such as "## ADDED Requirements" or move non-delta notes outside specs/.`。

但行为契约真相源 `openspec/specs/<cap>/spec.md` 主 spec **需要** Purpose 段（用户视角说明该 cap 提供的能力 / 数据流位置），否则 reviewer 看主 spec 失去入口；本仓所有 active cap 主 spec 都遵守 `# X Specification` + `## Purpose` + `## Requirements` 三段式。`openspec archive` 创建一个新 cap 主 spec 时仅 sync 已 ADDED 的 Requirement，**不**自动生成 Purpose 段（参见 commit `5b92009` archive port-context-tracking 后产出的主 spec 即无 Purpose；`175fd7e` 才是补 Purpose 的独立 docs commit）。

**决策**：本 PR 走"archive + 独立直接编辑"两步：

1. **archive 步**：`openspec archive split-session-display -y` 产出 3 个新 cap 主 spec（`openspec/specs/markdown/spec.md` / `openspec/specs/tool-viewer-routing/spec.md` / `openspec/specs/edit-diff-view/spec.md`），各自仅含 `# X Specification` 占位 header 与从 delta sync 来的 Requirement
2. **独立直接编辑步**（同 archive commit 内）：直接在 3 个新 cap 主 spec 内插入 `## Purpose` 段（每段 2-4 句中文，覆盖：cap 守护什么 / 与其它 cap 的边界 / 由谁消费），然后 `## Requirements` 段紧随其后

**为什么这是可接受的**：openspec/CLAUDE.md::硬约束 1 "禁止直接 Edit 主 spec" 的核心是"行为契约（SHALL/MUST 句）改动必须走 delta 让 reviewer 审"。Purpose 段是 cap 入口元描述，**不**含行为契约——属"非 delta 注解"，validator 也明确给了"move non-delta notes outside specs/"提示。文档命名 / 入口元描述用直接编辑添加，与 commit `175fd7e` 处理 14 个 cap Purpose 的工艺完全对齐，是仓内既有先例。

**3 个新 cap 的 Purpose 草稿**（archive 步骤后将插入到主 spec）：

#### markdown::Purpose

定义会话视图内 markdown 文本的统一渲染管线：所有用户消息正文 / AI 输出 / Thinking 与 Output 展开体 / Slash 命令 instructions / System 预格式化文本，经过同一套 Markdown → 语法高亮 → XSS 清洗 → 视口触发的懒渲染链路。本 spec 涵盖：fenced code block 高亮规则（声明语言 / 未声明语言 / 大块代码自动检测限制）、Mermaid 图表内嵌渲染、IntersectionObserver 驱动的视口触发与 `flushAll` 全文操作 fallback、紧急回滚开关。本 spec 仅约束 markdown 渲染管线本身的行为契约；具体由谁触发渲染（user 气泡 / AI 卡片 / Tool viewer / TeammateMessageItem 等）由各自所在 capability 守护。

#### tool-viewer-routing::Purpose

定义会话视图内工具调用的展开渲染契约：按 `toolName` 路由到专化 viewer（Read / Edit / Write / Bash / Default 等）的规则、tool row 头部的 token 估算与 duration 显示、折叠态不渲染重内容 / 展开按需 / 重展开复用缓存的性能契约、`outputOmitted=true` 路径下展开时机与 IPC 拉取节奏（按 viewer 是否消费 `exec.output` 分流）、大文本展开降级到轻量高亮、工具明细的 timing / waiting / failure 元信息显示。本 spec 既约束主会话工具列表，也约束 SubagentCard ExecutionTrace 内嵌的工具项。具体专化 viewer 内部的 DOM 结构与样式规约由消费方组件（`ReadToolViewer` / `EditToolViewer` / `WriteToolViewer` / `BashToolViewer` / `DefaultToolViewer`）承接；本 spec 守护工具调用 → viewer 路由 → 展开节奏的横向规则。

#### edit-diff-view::Purpose

定义 Edit 工具调用的 diff 视图渲染规则：以统一 diff 格式（context / added / removed 三类行 + old/new 双列行号 + Header 文件名/语言/统计）展示 `old_string` 与 `new_string` 的行级差异；按 `file_path` 推断语言后对 diff 行内容进行 highlight.js 语法高亮，无法推断或高亮失败时降级为纯文本 diff。本 spec 仅守护 EditToolViewer 的 diff 视图行为契约；专化 viewer 的路由规则（"toolName 为 Edit 时使用 EditToolViewer"）由 `tool-viewer-routing` capability 守护，diff 行不做重型语法高亮的展开节奏由 `tool-viewer-routing::大文本工具详情交互优先渲染` Requirement 守护。

## Risks / Trade-offs

| 风险 | 等级 | 缓解 |
|---|---|---|
| 字符级搬运过程引入 typo / 漏 Scenario | 中 | 4 个 cap Scenario 加和必须等于原 219；CI 跑 `scripts/check-spec-purity.sh` 拦异常；reviewer 抽样 diff 3-5 个 Requirement 字符级对照 |
| 边界灰区裁定与未来意图不符 | 低 | D-3 表格按"DOM 渲染主体 / 触发时机 / 数据消费形态"三原则裁定；裁定理由写明，后续若需重组只改一个新 cap 边界，不需 cascade 雪崩 |
| 跨 spec 引用更新漏 | 低 | grep 全 spec 仓库交叉验证，仅 tool-execution-linking 一处需要更新（agent-configs 本身指向 subagent 彩色体系仍归 session-display，不需改） |
| spec-purity baseline 噪声 | 中 | 12 个 Requirement body 内的历史污染（src 路径 / 内部 fn 名）从 session-display baseline mv 到新 cap baseline，统计上不增不减；baseline 文件需要在 PR 内同步更新（`tests/spec-purity-baseline.txt` 各 cap 行重新登记） |
| `openspec archive` 顺序坑（参 openspec/CLAUDE.md::硬约束 4） | 低 | 本 PR 不与其他 active change 并发修改 session-display 内同 Requirement；archive 时只有本 change 在动，无 race |
| edit-diff-view 粒度细仅 2 Requirement | 低 | issue #303::D-7 已决策不合并回 tool-viewer-routing；edit-diff-view 在长期路线上会承接 inline diff bookmark / diff 注释等更多 Edit 工具专属能力，独立 cap 利于之后扩展 |

## Migration Plan

1. 写 4 件套（proposal / design / tasks / 5 spec delta）
2. `openspec validate split-session-display --strict` 通过
3. 跑 `just preflight`（spec-validate + 不影响代码 / 测试，因为是纯 spec 改动）
4. design 阶段 codex 二审（命中 codex-usage.md::3 节"跨 ≥ 2 capability spec delta + UI 重构"两条规则）
5. commit + push + 开 PR
6. 并行启动 wait-ci + codex PR 二审 + spec-guide-reviewer
7. 三方都通过后 `openspec archive split-session-display -y`（一步原子完成 mv + sync 12 个 Requirement 到新 3 个主 spec / 1 个 MODIFIED 改回 session-display 主 spec / 1 个 MODIFIED 改回 tool-execution-linking 主 spec）
8. archive 同 commit 内直接编辑 3 个新 cap 主 spec（`openspec/specs/markdown/spec.md` / `tool-viewer-routing/spec.md` / `edit-diff-view/spec.md`）插入 D-6 草稿的 `## Purpose` 段
9. archive commit 作为 PR 最后一个 commit + 最后一次 wait-ci 全绿

## Open Questions

无。所有边界灰区已在 D-3 裁定。

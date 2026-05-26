## REMOVED Requirements

### Requirement: 工具专化查看器路由

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability。

**Migration**：行为契约 100% 不变；查看器路由由 `tool-viewer-routing` capability 内 `工具专化查看器路由` Requirement 守护。原 5 个 Scenario（Read 工具 / Edit 工具 / Write 工具 / Bash 工具 / 未知工具）全部字符级迁移到新 cap。

### Requirement: Markdown 渲染与代码高亮

**Reason**：本 Requirement 拆出到新建的 `markdown` capability。

**Migration**：行为契约 100% 不变；Markdown 渲染管线由 `markdown` capability 内 `Markdown 渲染与代码高亮` Requirement 守护。原 2 个 Scenario（代码块语法高亮 / XSS 防护）全部字符级迁移到新 cap。

### Requirement: Edit 工具 Diff 视图

**Reason**：本 Requirement 拆出到新建的 `edit-diff-view` capability。

**Migration**：行为契约 100% 不变；Edit 工具 diff 视图由 `edit-diff-view` capability 内 `Edit 工具 Diff 视图` Requirement 守护。原 4 个 Scenario（LCS diff 渲染 / Diff 行号 / Diff Header / 纯新增）全部字符级迁移到新 cap。

### Requirement: Mermaid 图表渲染

**Reason**：本 Requirement 拆出到新建的 `markdown` capability。

**Migration**：行为契约 100% 不变；Mermaid 渲染由 `markdown` capability 内 `Mermaid 图表渲染` Requirement 守护。原 4 个 Scenario（Mermaid 代码块渲染 / Code/Diagram 切换 / 渲染失败降级 / 主题适配）全部字符级迁移到新 cap。

### Requirement: Lazy markdown rendering for first paint performance

**Reason**：本 Requirement 拆出到新建的 `markdown` capability。

**Migration**：行为契约 100% 不变；lazy markdown 控制器由 `markdown` capability 内 `Lazy markdown rendering for first paint performance` Requirement 守护。原 9 个 Scenario（视口外不渲染 / 滚动进入视口渲染 / Mermaid 渲染时机 / 视口外不进入 highlight.js / 占位高度估算 / file-change 不打破 lazy / 紧急回滚 / flushAll 强制渲染 / flushAll 在回滚开关关闭时为 no-op）全部字符级迁移到新 cap。session-display 内"对话流容器及其 chunk / message 级稳定块容器 SHALL NOT 采用离屏估算高度占位"约束保留在 session-display `按 Chunk 类型渲染对话流` Requirement 内（已存在），与 markdown lazy 渲染契约互不重叠。

### Requirement: Lazy load tool output on expand

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability。

**Migration**：行为契约 100% 不变；展开时按需 IPC 拉取 tool output 由 `tool-viewer-routing` capability 内 `Lazy load tool output on expand` Requirement 守护。原 6 个 Scenario（折叠不触发 IPC / 展开按需拉 / 重复展开复用缓存 / 老后端 fallback / 嵌套 subagent 用 subagent sessionId / IPC 失败不阻塞）全部字符级迁移到新 cap。

### Requirement: Tool row displays approximate token count

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability。

**Migration**：行为契约 100% 不变；tool row token 估算与 duration 显示由 `tool-viewer-routing` capability 内 `Tool row displays approximate token count` Requirement 守护。原 2 个 Scenario（Bash 工具 row 显示 token 与 duration / missing output 工具仍显示 input token）全部字符级迁移到新 cap。

### Requirement: 大文本工具详情交互优先渲染

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability（归 viewer 路由 owner，因 viewer 路由本身决定展开节奏与 IPC 拉取时机）。

**Migration**：行为契约 100% 不变；大文本展开节奏与 viewer 路由判定由 `tool-viewer-routing` capability 内 `大文本工具详情交互优先渲染` Requirement 守护。原 11 个 Scenario（Read 大文本不阻塞 / Read 小中等保留高亮 / Write 大文本不阻塞 / Write 小中等保留高亮 / Edit diff 不重型高亮 / HTML 注入安全 / omitted output ready 后展开 / 嵌套 ExecutionTrace omitted output ready 后展开 / 不依赖 output 立即展开 / AIChunk 展开不主动 prefetch / 工具详情展开状态局部更新）全部字符级迁移到新 cap。

### Requirement: Tool detail timing and failure visibility

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability。

**Migration**：行为契约 100% 不变；工具明细 metadata 显示规则由 `tool-viewer-routing` capability 内 `Tool detail timing and failure visibility` Requirement 守护。原 4 个 Scenario（Completed tool 显示 duration / Pending tool 显示 waiting / Failed tool 显示原因 / Subagent trace 用同样 metadata）全部字符级迁移到新 cap。

### Requirement: Edit diff preview highlighting

**Reason**：本 Requirement 拆出到新建的 `edit-diff-view` capability。

**Migration**：行为契约 100% 不变；Edit diff 行语法高亮策略由 `edit-diff-view` capability 内 `Edit diff preview highlighting` Requirement 守护。原 4 个 Scenario（按 file extension 高亮 / 未知扩展 fallback / 纯新增显示内容 / Trailing newline 不产生 phantom 行）全部字符级迁移到新 cap。

### Requirement: Tool result expansion avoids eager heavy rendering

**Reason**：本 Requirement 拆出到新建的 `tool-viewer-routing` capability（归 viewer 路由 owner，因折叠态不渲染重内容是 viewer 行为）。

**Migration**：行为契约 100% 不变；折叠态不渲染重内容 / 首次展开按需 / 重展开复用缓存的策略由 `tool-viewer-routing` capability 内 `Tool result expansion avoids eager heavy rendering` Requirement 守护。原 3 个 Scenario（Collapsed tool 不渲染 heavy output / First expansion 按需渲染 / Re-expansion 复用 cached render）全部字符级迁移到新 cap。

### Requirement: 无语言代码块高亮自动检测限制

**Reason**：本 Requirement 拆出到新建的 `markdown` capability（归 markdown 渲染管线 owner，因未声明语言策略覆盖所有 markdown 调用点，不限工具 viewer 场景）。

**Migration**：行为契约 100% 不变；未声明语言代码块的安全渲染策略由 `markdown` capability 内 `无语言代码块高亮自动检测限制` Requirement 守护。原 3 个 Scenario（声明语言代码块保持高亮 / 未声明语言按 plaintext 渲染 / 大块代码不自动检测）全部字符级迁移到新 cap。

## MODIFIED Requirements

### Requirement: SubagentCard 在 ongoing 期间主动重拉 trace

SubagentCard MUST 监听 `(process.isOngoing, process.endTs, process.messagesTotalCount)` 三元组组成的版本指纹；当版本变化**且**该卡片处于用户已展开状态（`isExpanded === true`，即用户已点击展开按钮）**且**`process.messagesOmitted === true` 时，SHALL 自动调用 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉新 trace 并替换 `messagesLocal`。"已展开"判定 MUST 使用 `isExpanded` 而非 `messagesLocal !== null`——用 messagesLocal 判定会让首次展开期间（`ensureMessages` 的 `await` 进行中、`messagesLocal` 仍为 `null`）版本跳变后的新 fetch 不被触发，旧版本 fetch settle 后把 stale trace 写入 `messagesLocal`，UI 永久卡在旧快照（codex 二审 C1 发现）。

首次展开触发的 `ensureMessages` 与 effect 的版本主动重拉之间 SHALL 通过严格版本匹配协作：`ensureMessages` 在 IPC settle 时 MUST 检查 `currentVersion === fetchedVersion`，不匹配时 SHALL NOT 写入 `messagesLocal`（保持 `null`），让 effect 已发起的新版本 fetch 接管显示。早期实现里 `currentVersion === fetchedVersion || messagesLocal == null` 兜底语义 SHALL NOT 出现——`|| null` 兜底是 C1 的根本机制。

`getSubagentTrace` IPC 失败时 SHALL NOT 把 `messagesLocal` 写成空数组 `[]`——保留 `null` 让用户折叠重开时 `ensureMessages` 仍能命中 `messagesLocal == null` 通过 guard 重新尝试。早期实现把 `[]` 当作"显示空 trace"的兜底会让重试入口被永久封堵（codex 二审 C3 发现）。

未展开的 SubagentCard SHALL NOT 因版本变化主动发 IPC（仅清本地 stale 缓存或保持 `null`，等待用户下次展开时按既有 lazy 路径拉取），避免 ongoing 大会话内 N 个未展开卡片每次父 refresh 都触发 IPC 风暴。

同一 `process.sessionId` 同时收到多次版本变化 SHALL 通过 inflight 去重，但 inflight 复用 key MUST 为 `${sessionId}|${messagesVersion}` 联合 key，**不**仅按 sessionId 复用。理由：仅按 sessionId 复用时，旧版本（版本 N）的 Promise 在 pending 期间版本递增到 N+1，新触发的重拉若复用旧 Promise 会把版本 N 的旧 trace 写入 `messagesLocal`，且因 effect 认为"已在拉取中"而不再排第二轮——版本 N+1 的新 chunks 永远拿不到。等价替代实现：仅按 sessionId 复用但 Promise settle 后 SHALL 检查"当前版本 == fetch 时版本"，不等则视为 stale 并立即触发新一轮重拉。

#### Scenario: 已展开 ongoing subagent 在版本递增时主动重拉

- **WHEN** SubagentCard 已展开（`messagesLocal !== null`）且 `process.isOngoing === true`
- **AND** 父 session refresh 后 `process.messagesTotalCount` 从 5 变为 8
- **THEN** SubagentCard SHALL 自动调 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉，并把返回的 `Vec<Chunk>` 替换到 `messagesLocal`，UI 渲染的 ExecutionTrace SHALL 立即反映新增的 chunks，**无需**用户折叠重开

#### Scenario: ongoing 翻转到 done 时同步最终状态

- **WHEN** SubagentCard 已展开，`process.isOngoing` 从 `true` 翻转到 `false`（subagent 收尾）
- **AND** `process.endTs` 从 `null` 变为具体时间戳
- **THEN** SubagentCard SHALL 触发最后一次 `getSubagentTrace` 重拉，让 UI 同步到 subagent 完成态的完整 trace

#### Scenario: 未展开卡片不主动重拉

- **WHEN** SubagentCard 未展开（`isExpanded === false`），`process.messagesTotalCount` 在多次父 refresh 中递增
- **THEN** SubagentCard SHALL NOT 发 `getSubagentTrace` IPC；用户首次展开时 SHALL 走既有 lazy 路径拉一次最新 trace

#### Scenario: 首次展开期间版本跳变由 effect 接管

- **WHEN** 用户首次展开 SubagentCard：`isExpanded` 翻到 `true`，`ensureMessages` 启动 `getSubagentTrace`（版本 N，`messagesLocal` 仍为 `null`）
- **AND** pending 期间父 session refresh 让 `process.messagesTotalCount` 递增到 N+1
- **THEN** `$effect` SHALL 因 `isExpanded === true` 而触发新版本（N+1）的 `getSubagentTrace`，**不**因 `messagesLocal === null` 短路
- **AND** 旧版本（N）的 Promise settle 时 SHALL 严格判 `currentVersion === fetchedVersion`，不匹配则**不**写入 `messagesLocal`（保持 `null`），由新版本 fetch 接管显示

#### Scenario: IPC 失败后折叠重开能重试

- **WHEN** SubagentCard 已展开，`ensureMessages` 调 `getSubagentTrace` 但 IPC 抛错
- **THEN** `messagesLocal` SHALL 保持 `null`（**不**写成 `[]`）；`isLoadingTrace` 复位为 `false`
- **AND** 用户折叠（`isExpanded=false`）再展开（`isExpanded=true`）时，`ensureMessages` SHALL 因 `messagesLocal == null` 通过 guard 重新调 `getSubagentTrace`

#### Scenario: 同 sessionId 同版本并发触发 inflight 复用

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（尚未 settle）
- **AND** 同 sessionId 同版本 `"1|_|5"` 因 effect 重跑再次触发
- **THEN** 第二次 SHALL 复用第一次的 Promise（key `${sessionId}|1|_|5` 命中），SHALL NOT 并发发起第二次 IPC

#### Scenario: 同 sessionId 跨版本不复用旧 Promise

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（Promise A 尚未 settle）
- **AND** pending 期间版本递增到 `"1|_|8"`，新一轮重拉触发
- **THEN** 第二次 SHALL 视为新版本 fetch（key `${sessionId}|1|_|8` 不命中旧 inflight），SHALL 发起 Promise B；Promise A settle 时**不应**把版本 5 的旧 trace 写入 `messagesLocal`（fetch 时版本与当前版本不等，结果 SHALL 被丢弃或被 Promise B 的结果覆盖）

#### Scenario: 老后端缺 messagesTotalCount 字段降级

- **WHEN** 旧后端响应不含 `messagesTotalCount`（JSON 反序列化为 `undefined`）
- **THEN** 版本指纹三元组中 `messagesTotalCount` 视为 `undefined`，版本永远是常量，主动重拉 effect SHALL NOT 触发；行为退化为既有 lazy 路径（用户折叠重开才能看到新内容），SHALL NOT 报错或卡死

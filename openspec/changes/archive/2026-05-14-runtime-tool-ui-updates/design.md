## Context

当前桌面应用已经有 `cdt-watch` 递归监视 `~/.claude/projects/` 的文件变化，并通过 Tauri bridge 把 `file-change` 推给前端；SessionDetail 也已基于该事件刷新当前会话。但项目级入口（Sidebar / Dashboard / 项目选择框）对“新增项目目录或新增会话文件”没有完整刷新契约，导致运行中新项目不可见。

工具展示侧已经有 `ToolExecution`、专化 Tool Viewer、DiffViewer、Lazy markdown 与 subagent trace lazy load。现有问题集中在两类边界：工具结果展开时同步渲染成本过高，以及错误/耗时/diff 语言等元数据没有在所有展示路径中贯通。

## Goals / Non-Goals

**Goals:**

- 运行中新增项目或新 session 文件后，项目列表入口自动刷新。
- Edit 工具 diff 能预览并按文件语言进行高亮，失败时也保留可读信息。
- 工具明细统一显示耗时/等待状态，subagent ExecutionTrace 内的工具也遵循同一规则。
- 工具结果展开只在需要时渲染重内容，并复用 lazy markdown / highlight 路径降低卡顿。
- 工具失败原因在普通工具、专化 viewer、subagent trace 中一致可见。

**Non-Goals:**

- 不重做项目发现算法、git worktree 分组或 pin/hide 配置模型。
- 不引入新的 UI 虚拟列表框架；本轮只做展开路径的懒渲染与缓存。
- 不改变 `ToolExecution.output` 的 raw 数据来源，不读取 `toolUseResult.file.*` 来替代 raw `tool_result.content`。
- 不复刻 TS 原版 bug；若原版对失败原因或 diff 展示有缺陷，以本 change 的 spec 为准。

## Decisions

### D1: 用现有 file watcher 事件驱动项目列表刷新

项目新增属于 `file-watching` 与 `project-discovery` 的交界行为。优先扩展现有 watcher / Tauri bridge，让项目目录创建或项目下首个 `.jsonl` 创建时发出可识别的项目级刷新信号；前端收到后重新调用项目列表 API。

候选方案：

- 轮询 `listProjects`：实现简单但会持续消耗 I/O，且新增延迟不可控。
- 复用 file watcher：与现有自动刷新链路一致，事件驱动，改动面较小。

选择复用 watcher。若底层 notify 对新目录递归监听不稳定，启动时仍保留全量重扫作为刷新动作的权威来源，事件只负责触发。

### D2: 工具耗时由 `ToolExecution.start_ts/end_ts` 派生，不新增独立计时源

工具配对已经保存 `start_ts` 与 `end_ts`。UI 展示耗时时应优先用这两个字段计算；`end_ts=None` 时显示等待/未完成状态。subagent ExecutionTrace 里的工具项也使用同一 `ToolExecution` 数据，避免主会话与 subagent 口径分叉。

候选方案：

- 后端新增 `duration_ms` 到每个工具执行：前端简单，但会增加 IPC 字段与测试面。
- 前端基于已有时间戳派生：字段更少，口径透明，适合展示层逻辑。

选择前端派生；只有当现有时间戳未序列化或格式不稳定时，才补后端字段并同步 IPC contract。

### D3: 失败原因是工具输出的一种展示形态，不改变配对语义

`ToolExecution.is_error=true` 时，UI MUST 展示 `output` 中可读内容作为失败原因。结构化输出优先提取常见 `error` / `message` / `stderr` 字段；提取不到时展示格式化 JSON。这样不改变 `tool-execution-linking` 的配对算法，只补充展示契约。

候选方案：

- 后端规范化 `error_reason` 字段：更利于契约，但需要判断所有工具结构。
- UI 从 raw output 提取：更贴近现有数据模型，也能覆盖未知工具。

选择 UI 提取，同时在 spec 中要求 raw 内容不可丢失；若探索确认某些失败原因目前在解析阶段丢失，再补后端测试。

### D4: Edit diff 语言来自文件路径，渲染走 DiffViewer 局部高亮

Edit 工具 input 通常包含 `file_path`、`old_string`、`new_string`。DiffViewer 应从 `file_path` 推断语言，并对每行内容执行轻量高亮。Diff 算法仍保持现有 LCS 行级 diff，不把 Edit 展示改成完整 markdown。

候选方案：

- 把 diff 包成 fenced code block 交给 markdown renderer：实现快，但难保留 old/new 双列行号与 added/removed 样式。
- DiffViewer 自己按行高亮：能保留当前视觉结构，控制渲染成本。

选择 DiffViewer 局部高亮，并对未知语言降级为纯文本。

### D5: 大型工具结果只在展开后渲染，并缓存已渲染结果

卡顿主要来自展开时同步处理大文本、markdown、高亮或 JSON stringify。工具 BaseItem 折叠时不应创建重内容 DOM；首次展开后再渲染，且同一个 item 再次展开复用缓存结果。对超大文本继续使用 lazy markdown / viewport 触发，避免一次性高亮全量输出。

候选方案：

- 全局虚拟化整个会话流：收益大但侵入高，容易影响滚动语义。
- 工具展开体局部 lazy + 缓存：针对当前问题，风险小。

选择局部 lazy + 缓存。

## Risks / Trade-offs

- [Risk] notify 对运行中新建的深层目录事件在不同平台表现不一致 → Mitigation：项目刷新事件触发后统一调用 `listProjects` 全量重扫；测试覆盖目录创建与首个 `.jsonl` 创建两条路径。
- [Risk] 对每个 diff 行做语法高亮会增加展开成本 → Mitigation：只在展开 Edit 工具时高亮，未知语言纯文本，必要时限制超大 diff 高亮。
- [Risk] 失败原因结构化字段没有统一 schema → Mitigation：展示 raw fallback，保证“有内容可见”优先于精准字段抽取。
- [Risk] 前端派生耗时依赖时间戳格式 → Mitigation：先验证 IPC 中 `startTs/endTs` 可用；若不可用，补后端 camelCase 字段与 IPC contract。

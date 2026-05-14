## Context

Sidebar 初始加载当前先调用项目列表，再调用 `listAllSessions()` 获取会话列表。IPC 路径的 `list_sessions` 已经按设计返回骨架 `SessionSummary`，真实 `title`、`messageCount`、`isOngoing`、`gitBranch` 等 metadata 由后台任务通过 `session-metadata-update` 流式补齐。

性能瓶颈不在 metadata 补齐本身，而在“取得骨架列表”阶段仍会触发重复目录扫描：前端先请求 50 条，再用返回的 `total` 作为 `pageSize` 重拉一次全量；后端每次枚举项目下所有 `.jsonl` 时会逐文件读取 metadata 并排序。约 100 个会话时，这个二次扫描足以让 Sidebar 可见列表延迟到数秒级。

## Goals / Non-Goals

**Goals:**

- 保持 Sidebar 先显示骨架列表、metadata 后续流式补齐的体验。
- 避免前端为取得全量列表触发 `50 + total` 二次扫描。
- 降低后端骨架枚举的逐文件 I/O 成本，保持排序、分页、cursor、total 语义不变。
- 增加可重复的回归测试，覆盖多会话项目下的分页行为和后端枚举路径。

**Non-Goals:**

- 不改变 `SessionListResponse` JSON 字段或新增 Tauri command。
- 不改变 HTTP 同步完整列表的语义。
- 不在本 change 引入持久化索引、数据库或跨启动缓存。
- 不调整 metadata 解析策略、metadata cache 容量或 `session-metadata-update` 事件协议。

## Decisions

### D1: 前端改为分页累加，不再用 `total` 重拉全量

`listAllSessions()` SHALL 从第一页开始按固定 `pageSize` 递进 cursor，累加结果直到 `nextCursor` 为空。相比“首包 50 条 + 用 total 重拉全量”，分页累加不会重复请求已经拿过的第一页，也不会 abort 已启动的 metadata 扫描任务。

候选方案：

- 单次把默认 `pageSize` 提高到 500：实现最小，但仍把 UI 的“全部会话”假设编码成魔法上限，超过上限时行为退化。
- 后端新增 `list_all_sessions`：会扩大 IPC surface，当前需求不需要。
- 分页累加：复用既有 cursor 协议，兼容未来超过 100 个会话的项目。

### D2: 后端枚举优先减少重复 stat，而不是引入缓存

`ProjectScanner::list_sessions` SHALL 保持每次从目录读取真实文件列表，但在单次扫描内减少不必要的逐文件 metadata/stat 调用。实现优先使用 `DirEntry` 可提供的信息和较小的 per-file 工作量；如需并发，也只在 scanner 内部局部并发，不改变公共 API。

候选方案：

- 项目级缓存 + file watcher 失效：收益更大，但需要跨 `cdt-discover`、`cdt-watch`、`cdt-api` 协调失效，风险高于本次优化目标。
- 只改前端分页：能去掉二次扫描，但后端单次扫描仍偏慢。
- 单次扫描瘦身：低风险，行为可由现有排序/分页测试约束。

### D3: Metadata 后台任务触发保持在每次 `list_sessions` 后

前端分页可能调用多页 `list_sessions`。实现 SHALL 避免重复首包扫描，但不改变 `LocalDataApi` 现有“请求列表后启动 metadata 后台扫描”的契约。若分页导致多次调用，后端现有 per-key abort/generation 机制负责取消旧扫描；前端应通过减少请求次数和避免全量重拉来降低触发频率。

### D4: 验证以行为测试 + 可观测耗时为主

本 change 增加测试证明：

- `listAllSessions()` 在多页场景按 cursor 累加，不发起“用 total 重拉全量”的请求序列。
- `ProjectScanner::list_sessions` 保持按 `mtime` 倒序、cursor/limit/total 正确。

如果实现中新增内部计数或测试 hook，只能放在测试内或现有 mock 层，不进入生产 API。

## Risks / Trade-offs

- 分页累加可能增加请求次数：使用合理默认页大小（例如 100 或沿用后端上限）缓解；相比旧逻辑不会重复第一页。
- 后端使用 `DirEntry` metadata 在不同平台表现可能不同：保持 fallback 到现有 metadata 路径，测试覆盖 macOS/Linux/Windows 可接受行为。
- 不引入持久化索引意味着超大项目仍需目录枚举：本 change 先解决 100 级会话的明显慢路径，未来如遇 1000+ 会话再评估索引方案。

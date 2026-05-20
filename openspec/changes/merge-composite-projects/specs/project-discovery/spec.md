## REMOVED Requirements

### Requirement: Represent split subprojects with a stable composite identifier

**Reason**：同一 encoded 目录下不同 `cwd` 拆分为多个虚拟 project 在产品层与 `WorktreeGrouper` / `RepositoryGroup` 的 git identity 归并冗余冲突，且在 `LocalDataApi::get_session_detail` 路径上引发跨 project 全量扫描开销（仅为反解 composite id）。从用户视角出发，同一仓库的 worktree 或 monorepo 子目录被拆为多个 sidebar 项目反而碎片化体验。

**Migration**：
- 同一 encoded 目录下所有 session 始终归属同一 `Project`，`id = encoded base_dir`（不带 `::` 后缀）；不再产生 composite id。
- 不同 cwd 的区分由 `Session.cwd` 字段（新 Requirement `Expose session cwd for downstream display`）暴露给消费方，由 UI 通过 cwd badge 等手段展示。
- 配置中 `pinned_sessions: HashMap<String, Vec<PinnedSession>>` 的 key 含 composite id 时，由 `ConfigManager::load` 一次性 fold 为 base_dir 并合并去重（详见 `configuration-management` spec `Migrate composite project IDs in pinned sessions on load` Requirement）。`NotificationTrigger.repository_ids` 存的是 git-common-dir 绝对路径，与 composite 无关，不迁移。
- `SubprojectRegistry` / `COMPOSITE_SEPARATOR` 模块整体移除；下游 `cdt-api` 调用点（`is_composite` / `get_session_filter` / `get_cwd` / `get_entry`）随之删除。
- `ProjectPathResolver::resolve` 签名 SHALL 移除 `registry: &SubprojectRegistry` 参数，新签名为 `resolve(&self, project_id: &str, hint: Option<&Path>, session_paths: Option<&[PathBuf]>) -> Result<PathBuf, DiscoverError>`；原 `registry.get_cwd` short-circuit 路径删除，解析顺序退化为 `cache → hint → session-jsonl-cwd → decode_path(base_dir)`。所有调用点（`worktree_grouper.rs` 测试 + 其它）SHALL 同步去掉 registry 实参。
- `agent-configs` capability 的 scan 入口 SHALL 改为按一个 project 下所有 session 的 `cwd` 去重集合扫描（详见 `agent-configs` spec MODIFIED Requirement），避免合并后丢失非代表 cwd 的 agent 配置。

## ADDED Requirements

### Requirement: Expose session cwd for downstream display

系统 SHALL 在 `Session`（`cdt-core::Session`，IPC 序列化形态）中暴露 `cwd: Option<String>` 字段，值取自该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值；该字段为空（jsonl 不含 cwd）时 SHALL 为 `None`。序列化 SHALL 使用 camelCase（`cwd`），并在为 `None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略输出。

`ProjectScanner::scan_project_dir` SHALL 在产生 `Session` 时把 `extract_session_cwd` 的结果直接写入 `Session.cwd`；该 cwd 提取沿用现有 head-read（仅读 jsonl 前 `SESSION_HEAD_LINES` 行）+ `FILE_READ_CONCURRENCY` 信号量限流路径，**不**得为获取 cwd 而触发全文件读取（除非 head 不含 cwd 字段时按现有 `extract_session_cwd` SSH fallback 路径回滚）。

#### Scenario: 单 cwd session 暴露 cwd 字段

- **WHEN** 一个 jsonl session 首条消息 `cwd = "/Users/foo/myrepo"`
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `Some("/Users/foo/myrepo")`
- **AND** IPC 序列化结果 SHALL 包含 `"cwd": "/Users/foo/myrepo"`

#### Scenario: 缺 cwd session 暴露 None

- **WHEN** 一个 jsonl session 所有消息均不含 `cwd` 字段
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `None`
- **AND** IPC 序列化结果 SHALL 省略 `cwd` 键（不出现 `"cwd": null`）

#### Scenario: 同一 encoded 目录多 cwd 的 session 各自暴露真实 cwd

- **WHEN** 一个 encoded 目录 `D` 下含两条 session，cwd 分别为 `/a/b` 与 `/a/c`
- **THEN** 系统 SHALL 输出**一条** `Project`（`id = D`，不再拆分），其 `sessions` 列表两条目分别带 `cwd = Some("/a/b")` 与 `cwd = Some("/a/c")`

#### Scenario: 提取 cwd 不触发全文件读

- **WHEN** 一个 session jsonl 文件大小 100 MB，cwd 在前 20 行内
- **THEN** 系统 SHALL 仅通过 head-read（`FileSystemProvider::read_lines_head`）拿到 cwd
- **AND** SHALL NOT 触发对该文件的 `read_to_string`

## MODIFIED Requirements

### Requirement: Compare paths case-insensitively on Windows

系统 SHALL 在所有路径比较点（HashMap/BTreeMap key、HashSet 元素、`starts_with` / `eq` 判定、hash 输入）使用统一的跨平台规范化 helper，使**Windows 平台**上仅大小写不同的两条路径被视为相等，**非 Windows 平台**保持字节精确比较。

规范化 helper SHALL 由 `cdt-discover::path_compare` 模块统一提供，是整个 workspace 中跨平台路径比较的唯一来源；任何其它 crate 需要做路径比较 / hash 时 SHALL 引用该模块的公开函数，**不得**自行实现 lowercase / equality 逻辑。规范化策略 SHALL 使用 ASCII lowercase（与 TS 原版 `pathValidation.ts::normalizeForCompare` 行为对齐），不做 Unicode 大小写折叠。

`ProjectPathResolver` 的内部 cache key（encoded `project_id`）SHALL 在插入与查询前都经过此规范化。

#### Scenario: Windows 上同一路径不同大小写归一

- **WHEN** 在 Windows 平台运行，两条 session 的 `cwd` 字段分别为 `C:\Users\Alice\app` 与 `c:\users\alice\app`
- **THEN** `ProjectPathResolver` SHALL 把两条 session 视为同一 project

#### Scenario: 非 Windows 平台保持精确比较

- **WHEN** 在 Linux 或 macOS 平台运行，两条 session 的 `cwd` 字段分别为 `/Users/alice/App` 与 `/users/alice/app`
- **THEN** `ProjectPathResolver` SHALL 把两条 session 视为不同 project

#### Scenario: 跨大小写命中同一 ProjectPathResolver 缓存

- **WHEN** 在 Windows 平台运行，调用方先用 encoded `project_id = "-C:-Users-Alice-app"` 触发解析并写 cache，再用 `"-C:-users-alice-app"`（同一目录、不同大小写）查询
- **THEN** `ProjectPathResolver::resolve` SHALL 命中第一次的 cache 条目，返回相同 `PathBuf`，不重新走文件系统扫描

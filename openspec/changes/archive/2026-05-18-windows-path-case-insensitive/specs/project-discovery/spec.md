## ADDED Requirements

### Requirement: Compare paths case-insensitively on Windows

系统 SHALL 在所有路径比较点（HashMap/BTreeMap key、HashSet 元素、`starts_with` / `eq` 判定、hash 输入）使用统一的跨平台规范化 helper，使**Windows 平台**上仅大小写不同的两条路径被视为相等，**非 Windows 平台**保持字节精确比较。

规范化 helper SHALL 由 `cdt-discover::path_compare` 模块统一提供，是整个 workspace 中跨平台路径比较的唯一来源；任何其它 crate 需要做路径比较 / hash 时 SHALL 引用该模块的公开函数，**不得**自行实现 lowercase / equality 逻辑。规范化策略 SHALL 使用 ASCII lowercase（与 TS 原版 `pathValidation.ts::normalizeForCompare` 行为对齐），不做 Unicode 大小写折叠。

`ProjectPathResolver` 的内部 cache key（encoded `project_id`）、`ProjectScanner` 内部按 cwd 聚类的 bucket key、`SubprojectRegistry::compose_id` 的 hash 输入 SHALL 在插入与查询前都经过此规范化。

#### Scenario: Windows 上同一路径不同大小写归一

- **WHEN** 在 Windows 平台运行，两条 session 的 `cwd` 字段分别为 `C:\Users\Alice\app` 与 `c:\users\alice\app`
- **THEN** `ProjectPathResolver` 与 `SubprojectRegistry` SHALL 把两条 session 视为同一 project / 同一 subproject
- **AND** `compose_id` 对两个 cwd 输入 SHALL 返回相同的 8 字符十六进制后缀

#### Scenario: 非 Windows 平台保持精确比较

- **WHEN** 在 Linux 或 macOS 平台运行，两条 session 的 `cwd` 字段分别为 `/Users/alice/App` 与 `/users/alice/app`
- **THEN** `ProjectPathResolver` 与 `SubprojectRegistry` SHALL 把两条 session 视为不同 project / 不同 subproject
- **AND** `compose_id` 对两个 cwd 输入 SHALL 返回不同的 8 字符十六进制后缀

#### Scenario: 跨大小写命中同一 ProjectPathResolver 缓存

- **WHEN** 在 Windows 平台运行，调用方先用 encoded `project_id = "-C:-Users-Alice-app"` 触发解析并写 cache，再用 `"-C:-users-alice-app"`（同一目录、不同大小写）查询
- **THEN** `ProjectPathResolver::resolve` SHALL 命中第一次的 cache 条目，返回相同 `PathBuf`，不重新走文件系统扫描

## Why

`cdt sessions list --since 2026-06-07 --until 2026-06-08` 基于文件 mtime 单点过滤，晚间活跃但跨过午夜结束的会话被归到"第二天"而系统性遗漏。实测 2026-06-07 的会话查询漏掉 8/17（47%），包括两个 $260+ 的大会话——用户说"昨天的会话"意味着"昨天有活动的会话"，不是"文件最后修改时间恰好落在昨天"。

## What Changes

- `FsMetadata` 新增 `created: Option<SystemTime>` 字段，从 `std::fs::Metadata::created()` 获取文件创建时间（birthtime）
- `Session` / `SessionSummary` 新增 `created` 字段（epoch ms），来源为文件 birthtime，fallback 到 mtime
- `QueryFilter` 的 `--since/--until` 过滤语义从 mtime 单点匹配改为 `[created, mtime]` 区间交集：`session.created <= until AND session.mtime >= since`
- `QueryEngine::list_sessions_cross_project` 的 group 级剪枝同步适配

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `fs-abstraction` — `FsMetadata` 新增 `created` 字段
- `project-discovery` — `Session` struct 新增 `created` 字段；文件扫描时提取 birthtime
- `ipc-data-api` — `SessionSummary` 新增 `created` 字段；`timestamp` 语义不变（仍为 mtime，用于排序和展示）
- `cli-output` — `--since/--until` 过滤语义从 mtime 单点改为区间交集

## Impact

- **后端**：`cdt-fs` / `cdt-discover` / `cdt-core` / `cdt-api` / `cdt-query` / `cdt-cli` 六个 crate + `cdt-ssh` / `cdt-watch`（`FsMetadata` 构造点需补 `created` 字段）
- **前端**：`SessionSummary` TypeScript 接口加 `created` 字段（`serde(default)` 向后兼容）；前端不需要改消费逻辑
- **IPC 契约**：新增字段，非 BREAKING（`#[serde(default)]`）
- **性能**：零额外 I/O——`created` 从已有 `stat` 系统调用的 `metadata.created()` 获取
- **平台兼容**：macOS/Windows 原生支持 birthtime；Linux ext4/btrfs/xfs + kernel 4.11+ 支持 `statx`；不支持时 fallback 到 mtime（退化为当前行为）

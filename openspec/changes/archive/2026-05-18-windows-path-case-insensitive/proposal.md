## Why

Windows 文件系统**大小写不敏感**——同一目录在不同来源（用户输入 / JSONL `cwd` 字段 / 配置文件 / `notify` 回调）可能以不同大小写出现：`C:\Users\Alice` vs `c:\users\alice`。Rust port 当前所有路径比较都走默认 `PartialEq` / `starts_with` / 哈希——大小写敏感，导致 Windows 用户的真实数据匹配失败：

- `cdt-watch::FileWatcher` 用 `path.starts_with(&self.projects_dir)` 路由事件，`notify` 回调返回的路径若大小写与 `dunce::canonicalize` 的 `projects_dir` 不一致，事件被丢弃 → 前端永不刷新
- `cdt-discover::ProjectPathResolver` 的 cache 用 `HashMap<String, PathBuf>` 以 encoded `project_id` 为 key，同一项目的两条 session 因 `cwd` 大小写差被切成两个 cache 条目
- `subproject_registry::compose_id` 与 `project_scanner` 的 cwd bucket 同样按字符串字节比较，Windows 上同一项目被误拆成两个 subproject

TS 原版 `pathValidation.ts::normalizeForCompare` 在 `process.platform === 'win32'` 时 `path.normalize().toLowerCase()` 已处理这个问题——本 change 把这个语义补回 Rust port。

`openspec/followups.md` 已记录该 coverage-gap，本 change 关闭该项。

## What Changes

- **新增** `cdt-discover::path_compare` 模块，导出三个 helper：
  - `paths_equal(a: &Path, b: &Path) -> bool`——Windows 大小写不敏感、Unix 精确比较
  - `path_starts_with(haystack: &Path, prefix: &Path) -> bool`——同语义的前缀匹配
  - `normalize_path_for_compare(p: &Path) -> Cow<'_, Path>`——返回比较用规范化形式（Windows 全转小写、Unix 原样），用于 HashMap key / hashing
- **接入**全部 7 处 MUST FIX 触点：
  - `cdt-watch::FileWatcher::route_event`（3 处 `starts_with` + `strip_prefix`）
  - `cdt-watch::FileWatcher::known_projects`（HashSet 元素去重）
  - `cdt-discover::ProjectPathResolver::cache`（key 规范化）
  - `cdt-discover::ProjectScanner` 的 cwd bucket（`BTreeMap` key 规范化）
  - `cdt-discover::SubprojectRegistry::compose_id`（hash 输入规范化）
  - `cdt-config::mention` 路径校验（2 处 `starts_with`）
- **不改**：`cdt-api` 内部 cache（`parsed_message_cache` / `session_metadata`）—— path 来自 `local.rs` 直接 `projects_dir.join(project_id).join(session_id)` 构造（绕过 resolver），但 `project_id` 是 IPC 输入、前端从 scanner 输出原样回传，**事实上同源**；Windows 上若 IPC 同一目录前后用不同大小写呼叫，会 cache miss + rebuild 一次，**不影响正确性**只多一次 parse。本 change 不纳入以缩小改动半径，必要时另开 change 处理
- **跨平台行为不变**：Unix / macOS 路径比较语义保持精确（按字节比较），仅 Windows 走 ASCII 小写规范化

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `project-discovery`: 新增 Requirement，规范"路径比较时 Windows 平台 SHALL 大小写不敏感"，覆盖 `ProjectPathResolver` cache、`ProjectScanner` cwd bucket、`SubprojectRegistry::compose_id` 三处
- `file-watching`: 新增 Requirement，规范"watcher 事件路由 SHALL 在 Windows 上做大小写不敏感前缀匹配"，覆盖 `route_event` 与 `known_projects` 去重

## Impact

- **代码**：cdt-discover 新增 `path_compare.rs` 模块（~60 行 + 单测）；cdt-watch / cdt-discover / cdt-config 共 ~7 处 callsite 切到新 helper（~30 行改动）
- **API**：无 IPC 字段变化、无 Tauri command 协议变化；新 helper 是 cdt-discover 公开 API，会被 cdt-watch / cdt-config 依赖
- **依赖**：无新外部 crate。本 change 选 **ASCII lowercase**（不是 TS `toLowerCase()` 的 Unicode default case mapping 完全等价）作为有意近似——实务路径绝大多数是 ASCII 字符，不引入新依赖且可枚举测试。非 ASCII 用户名（土耳其 `İ`/`ı` 等）偏离，待真实用户报告后再升级 Unicode 方案。设计权衡详见 `design.md::D2`
- **跨平台**：Unix / macOS 行为完全不变；仅 `#[cfg(target_os = "windows")]` 分支启用 ASCII lowercase 规范化
- **followups**：关闭 `openspec/followups.md::Windows 平台::[coverage-gap] Windows 路径大小写不敏感比较`

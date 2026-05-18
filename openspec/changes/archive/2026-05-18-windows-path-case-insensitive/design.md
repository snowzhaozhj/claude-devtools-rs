## Context

`openspec/followups.md::Windows 平台::[coverage-gap] Windows 路径大小写不敏感比较` 记录的是一个 port-fidelity gap：TS 原版 `pathValidation.ts:65-72` 在 Windows 上做 `path.normalize().toLowerCase()`，Rust port 当前缺失该规范化，导致同一路径以不同大小写出现在两端时比较失败。

Explore 子代理对 `cdt-core / cdt-parse / cdt-analyze / cdt-discover / cdt-watch / cdt-config / cdt-ssh / cdt-api / cdt-cli + src-tauri/` 全量扫描，分类如下：

**MUST FIX**（用户输入 / JSONL 数据 / `notify` 回调来源不一致的比较）—— 7 处：
- `cdt-watch::FileWatcher::route_event` L132/136/153：`path.starts_with(&projects_dir)` / `starts_with(&todos_dir)` / `path.strip_prefix(&projects_dir)`
- `cdt-watch::FileWatcher::known_projects` L45/227：`HashSet<PathBuf>` 用 `projects_dir.join(project_id)` 当元素，`PathBuf` 默认 `Hash` 大小写敏感
- `cdt-discover::ProjectPathResolver::cache` L33：`HashMap<String, PathBuf>` key 是 encoded `project_id`，源数据 `cwd` 字段大小写漂移
- `cdt-discover::ProjectScanner` L248/276：`BTreeMap<String, CwdBucket>` 按 raw cwd 字符串聚类
- `cdt-discover::SubprojectRegistry::compose_id` L59-61：SHA-256(`cwd.to_string_lossy().as_bytes()`)
- `cdt-config::mention` L100/105：`normalized.starts_with(&claude_dir)` / `starts_with(root)` 校验用户输入路径

**MAY FIX**（保留大小写漂移代价，本 change 不改）—— 2 处：
- `cdt-api::parsed_message_cache::HashMap<PathBuf, _>` L35
- `cdt-api::session_metadata::HashMap<PathBuf, MetadataCacheEntry>` L304
- 这两处的 path 来自 `cdt-api::ipc::local.rs::{L624-650, L824-826, L1135-1148}` 直接 `projects_dir.join(project_id).join(session_id)` 构造，**绕过** resolver。`project_id` 是 IPC 输入（前端从 `list_projects` 输出选并原样回传），来源同 scanner，**事实上稳定**——但 spec 不强保证 IPC 用同一字符串呼叫同一 cache 条目。Windows 上若 IPC 同一会话先以 ID-A 后以 id-a 呼叫，会触发 cache miss + rebuild，**不影响正确性**只多一次 parse。本 change 不纳入规范化以缩小 blast radius；若日后实测 cache miss 发生，再开新 change 把 cache key 也走 helper。

**NOT NEEDED**：cdt-core / cdt-parse / cdt-analyze / cdt-ssh / cdt-cli / src-tauri 无路径比较点；`cdt-ssh::config_parser::to_lowercase` 是 SSH 关键字，不属于路径。

## Goals / Non-Goals

**Goals:**
- Windows 上同一路径的不同大小写形式 SHALL 在所有 MUST FIX 触点中视为相等
- Unix / macOS 行为**完全不变**：仍按字节精确比较
- 新增的 helper `paths_equal` / `path_starts_with` / `normalize_path_for_compare` 是整个 workspace 的唯一比较入口，禁止 callsite 自行实现
- 所有 callsite 切换 helper 后 clippy / fmt / 既有测试 0 回归
- 新增至少 4 个跨平台单测（Windows / Unix 各一对正反）证明语义

**Non-Goals:**
- **不**做 Unicode 大小写折叠（`char::to_lowercase`）。Windows NTFS / ReFS 的 upcase 表是 OEM/Unicode 表，但实务路径里出现非 ASCII 字符的几乎只有用户名（中文 / 日文等），且这些字符在 NTFS 默认 upcase 表里通常仍按原样比较。ASCII lowercase 已覆盖 99% 真实场景（drive letter / `Users` / `Program Files` 等），且不引入新依赖
- **不**改 `cdt-api` 内部 cache（MAY FIX）—— 同源数据，无需在 cache 端再加一层规范化
- **不**改 `dunce::canonicalize` 调用——该函数只规范 UNC 前缀，不动大小写；保留对 symlink / `..` 的处理
- **不**引入新外部 crate

## Decisions

### D1：helper 放 cdt-discover 而不是 cdt-core

**选择**：在 `cdt-discover::path_compare` 新模块导出三个公开函数。

**候选方案**：
- (A) 放 `cdt-core`：理论上所有 crate 都可以依赖
- (B) 放 `cdt-discover`：已有 `path_decoder` / `project_path_resolver` 等路径相关工具，且被 cdt-watch / cdt-config / cdt-api 都依赖
- (C) 各 crate 自行实现 helper

**取舍**：
- 选 (B)。`cdt-core` 是纯数据类型 crate，不引入比较逻辑；`cdt-discover` 已是路径工具的"中心"——`path_decoder::encode_path` 已被 `cdt-config::claude_md` 跨 crate 使用，新 helper 沿用同模式。
- 拒 (C)：违反 followups 教训"`encode_path` 私有副本导致 Windows auto-memory 查找失败"，唯一实现是硬约束。

### D1b：为什么不用现成 crate

调研了 Rust 生态主要候选：

| crate | 用途 | 不适用原因 |
|---|---|---|
| `dunce` | Windows UNC 前缀去除 | 已用于 `FileWatcher::with_paths` 处理 `\\?\` 前缀；不做大小写 |
| `path-clean` / `path-absolutize` | 处理 `..` / `.` 组件 | 不做大小写 |
| `samefile` | device/inode 判同一文件 | 要求文件**真存在**——本 change 场景里 `notify` deleted 路径、cache key（逻辑标识）、JSONL 历史 `cwd` 都可能没真文件 |
| `unicase` / `caseless` | Unicode 大小写折叠 | 只覆盖 `&str`；且 NTFS 真实用 OEM upcase 表与 Unicode 折叠**不等价**，换它也只是从 ASCII 近似换 Unicode 近似 |
| 标准库 `Path::eq` / `starts_with` | 字节精确 | 不做大小写 |

Windows 上"真正等价文件系统"的比较需要 Win32 `CompareStringOrdinal` 或 NTFS `\$UpCase` 表（`windows-rs` + 平台分叉）——复杂度远高于本 change 范围。TS 原版 `pathValidation.ts::normalizeForCompare` 也是 8 行 `path.normalize().toLowerCase()`，同档位 ASCII 近似（JS `toLowerCase` 名义上是 Unicode default case mapping，但实务路径不出 ASCII）。

本 change ~30 行 `Cow<Path>` + ASCII lowercase 的成本：0 新依赖、单测可枚举、与 TS 行为档位一致；真出现非 ASCII 用户名报错时升级到 `unicase` 的破坏面只在 `cdt-discover::path_compare` 单 module。

### D2：Windows 规范化用 ASCII lowercase 而非 Unicode 折叠

**选择**：`#[cfg(target_os = "windows")]` 分支用 `c.to_ascii_lowercase()` 逐字节处理；其它平台直接返回原 `Path`（`Cow::Borrowed`）。

**候选方案**：
- (A) ASCII lowercase（`u8::to_ascii_lowercase`）
- (B) Unicode 简单折叠（`char::to_lowercase().collect()`）
- (C) NTFS upcase 表（移植 Windows kernel 的 upcase.nls）

**取舍**：
- 选 (A)。**注意**：TS 原版 `pathValidation.ts::normalizeForCompare` 用 `path.toLowerCase()`，JS 该方法走 Unicode default case mapping（ECMA-262）；Rust ASCII lowercase **不是 TS 的完全等价物**——非 ASCII 路径（土耳其 `İ`/`ı`、希腊 `Σ`/`σ`/`ς`、中日韩等）在两端会偏离。本 change 选 ASCII 是**有意为之的近似**：(1) 实务路径字符集是 ASCII drive letter + `Users` + `Program Files` + ASCII 用户名占绝大多数；(2) Windows NTFS 自身的 upcase 表是 OEM/Unicode，与 ECMA-262 又不完全等价，TS 在 NTFS 上同样不是真"file system 等价"；(3) 不引入新依赖、单测可枚举。
- 拒 (B) `char::to_lowercase`：会产生多字符序列（土耳其 i / 德语 ß），并且 OS file system 不见得用 Unicode 折叠表，反而更容易出 false positive。若未来真实用户报告非 ASCII 用户名漂移，再升级到 Unicode 方案。
- 拒 (C)：增加 maintenance cost，且与 TS 原版偏离方向相反。

### D3：HashMap key 怎么规范化

**选择**：在**插入**与**查询**的两处都先调 `normalize_path_for_compare`，存原始 `PathBuf` 但用规范化字符串当 key（或独立 wrapper struct `NormalizedPathKey(PathBuf)` 含 `Hash + Eq` 大小写不敏感实现）。

**候选方案**：
- (A) 包装类型 `NormalizedPathKey(PathBuf)`，实现 `Hash` / `Eq` 跨平台规则
- (B) 调用前手工 `to_compare_form()` 转换；存的还是 `String` / `PathBuf`
- (C) 用 `BTreeMap` 配合自定义 `Ord` 实现

**取舍**：
- 选 (B)。callsite 数量小（5 处 HashMap/BTreeMap 插入查询），手工 normalize 是显式的；包装类型 (A) 会让 `HashMap<NormalizedPathKey, V>` 的迭代 / 序列化等下游 API 变复杂。
- 把 normalize 函数本身设计成 `fn normalize_path_for_compare(p: &Path) -> Cow<'_, Path>`：Unix 借出原 `Path`、Windows 返回 lowercase 后的 `PathBuf`。callsite `.normalize_path_for_compare().into_owned()` 拿 owned key，最小心智负担。

### D4：跨平台测试怎么写

**选择**：用 `#[cfg(target_os = "windows")]` 与 `#[cfg(not(target_os = "windows"))]` 双分支测试，**不**用 mock。

**候选方案**：
- (A) 跨平台测试，按 `target_os` 写期望值
- (B) 抽象一个 `Platform` trait，注入测试用的 `WindowsPlatform`
- (C) 只在 Windows CI 上跑，Unix 跳过

**取舍**：
- 选 (A)。helper 内部就是 `cfg` 切换，行为本身依赖编译目标。在 Unix 测试断言"两个不同大小写的 path 不相等"，在 Windows 测试断言"相等"——双向证明 cfg 分支正确。
- 已有 `windows-platform-support` change archive 走类似模式，CI 矩阵已含 Windows runner。

## Risks / Trade-offs

- **风险 1：ASCII lowercase 对非 ASCII 路径仍敏感** → 影响中文 / 日文用户名场景。Mitigation：followups 留一条"Unicode 折叠待真实用户报告后再升级"，本 change 不阻塞；TS 原版同样不做完整 Unicode 折叠。
- **风险 2：内部一致性 cache 的"假阴性"误改** → 若把 `cdt-api::parsed_message_cache` 也按 normalize 改了，可能因为 resolver 端没规范化而出现 cache 命中漂移。Mitigation：本 change 明确**不改** MAY FIX 触点，资源同源原则。
- **风险 3：`Cow<'_, Path>` 生命周期与 callsite 集成摩擦** → 部分 callsite 已经 `.to_string_lossy()` 转 `String`，要做 `let key = normalize_path_for_compare(&p).to_string_lossy().into_owned();`。Mitigation：在 helper 模块同时导出 `normalize_path_string_for_compare(s: &str) -> Cow<'_, str>` 兼顾"已经是 String"的 callsite。
- **风险 4：`SubprojectRegistry::compose_id` 改 hash 输入会让历史 ID 漂移** → Windows 用户升级后旧 composite ID 失效，前端 UI state（pinned session 等）丢映射。Mitigation：composite ID 仅在 runtime 内部使用、不持久化（已查 `subproject_registry.rs` 与 `cdt-config` 调用——pinned session 用 plain `session_id`，不绑 composite）。Windows 用户首次升级 cold-start 后会按新规则重建 ID，对 UI 透明。

## Context

PR-A 定义 `BackendPolicy` 雏形（4 个 IPC initial-load 相关 primitive 字段）+ 三个 `const fn` 构造器，spec 钉死"业务代码尚未消费 BackendPolicy（PR-E 才接入）"。PR-D 修通 fs trait 直调 / cache wrapper / SSH SkeletonThenStream 等 23 处 algorithm 分叉后，沉淀了 **6 处** policy 分叉注释（每处都钉了"PR-E lift to BackendPolicy::xxx"），等本 PR wire。

**当前 6 处分叉**（grep `crates/cdt-api/src/ipc/local.rs` @ main HEAD `5d0207a`）：

| line | callsite | 现状 | 目标字段 |
|---|---|---|---|
| 2206-2215 | `get_project_memory` SSH early-return empty `ProjectMemory` | `if fs.kind() == Ssh { return empty }` | `BackendPolicy::supports_memory: bool` |
| 2239-2244 | `read_memory_file` SSH 返 not_found | `if fs.kind() == Ssh { return not_found }` | 同上 |
| 2315-2323 | `get_session_detail` subagent 扫描 SSH 返 `Vec::new()` | `let candidates = if is_remote { vec![] } else { scan... }` | `BackendPolicy::supports_subagent_scan: bool` |
| 2331-2338 | `get_session_detail` is_ongoing stale 5min check SSH 跳过（远端 mtime/local clock 跨 domain）| `if messages_ongoing && !is_remote { stale_check } else { messages_ongoing }` | `BackendPolicy::stale_check_strategy: StaleCheckStrategy` |
| 2738-2739 | `search` 用 `SearchConfig::from_fs_kind(fs.kind())` 选 SSH stage-limit | inline `from_fs_kind` | `BackendResolvers::search_config: SearchConfig` |
| 3151-3170 | `list_repository_groups` Local 用 `LocalGitIdentityResolver` / SSH 用 `NoopGitIdentityResolver` | `let groups = if is_remote { Noop } else { Local }` | `BackendResolvers::git_identity_resolver: Arc<dyn GitIdentityResolver>` |

**关键约束（cdt-fs purity）**：`openspec/specs/fs-abstraction/spec.md` Scenario "cdt-fs 不依赖业务 crate" 钉死 cdt-fs 允许 deps 仅限 `tokio / async-trait / thiserror / tracing`。所以 `Arc<dyn GitIdentityResolver>`（cdt-discover）+ `SearchConfig`（cdt-discover）+ `RepositoryIdentity`（cdt-core）**不能**作为 `BackendPolicy` 字段直接塞进 cdt-fs。

## Goals / Non-Goals

**Goals**：
- `BackendPolicy` 加 3 个 primitive 字段（`supports_memory: bool` / `supports_subagent_scan: bool` / `stale_check_strategy: StaleCheckStrategy`）+ 新 `StaleCheckStrategy` enum
- `BackendPolicy` 保持 `Copy + PartialEq + Eq + Clone + Debug + const fn 构造器`
- cdt-api 新增 `BackendResolvers { search_config, git_identity_resolver }` 持非 Copy/dyn 字段
- `LocalDataApi` 加 `active_fs_and_policy()` helper 返五元组 `(fs, projects_dir, ctx, Arc<BackendPolicy>, Arc<BackendResolvers>)`，调用方一次 await 拿同快照
- 6 处 callsite 改完后业务代码不直接 `match fs.kind()` —— 走 policy/resolvers 字段
- 加 grep 不变性测试：`crates/cdt-api/src/ipc/local.rs` 内 `fs.kind() ==` / `let is_remote =` 出现次数 SHALL ≤ 阈值（仅允许顶层派生）
- 全程不破 perf 基线（`bash scripts/run-perf-bench.sh --runs 5` 四维 verify）

**Non-Goals**：
- **不**实现 SSH-aware clock skew compensation（留 PR-G/follow-up）
- **不**前端展示"SSH 不支持 memory" i18n 文案（前端 polish 留 follow-up）
- **不**改 `BackendPolicy::for_http()`（HTTP backend 暂不接 SSH-specific 行为，按 Local 行为兜底）
- **不**重构 `LocalGitIdentityResolver` / `NoopGitIdentityResolver` 内部实现
- **不**改 IPC 字段 / 前端契约
- **不**做 PR-D2 `read_dir_with_metadata` batch 优化（并行 PR）
- **不**做 PR-F SFTP message-id pipeline

## Decisions

### D1: BackendPolicy 在 cdt-fs 内只放 primitive 字段；`Arc<dyn>` / `SearchConfig` 放 cdt-api 的 `BackendResolvers`

**问题**：6 处分叉中 5 处目标字段都是 primitive（bool / enum），但 `git_identity_resolver` 是 `Arc<dyn GitIdentityResolver>`（trait 在 cdt-discover），`search_config` 是 `SearchConfig`（struct 在 cdt-discover）。直接放进 `cdt-fs::BackendPolicy` 字段需要 cdt-fs 依赖 cdt-discover，**违反** `openspec/specs/fs-abstraction/spec.md` Scenario "cdt-fs 不依赖业务 crate"。

**修法**：两层拆分
- `cdt-fs::BackendPolicy`：3 个 primitive 字段（bool / bool / enum），保留 `Copy + Eq` + `const fn` 构造器
- `cdt-api::ipc::backend_resolvers::BackendResolvers`：`SearchConfig` + `Arc<dyn GitIdentityResolver>` 字段，工厂方法 `for_local()` / `for_ssh()` / `from_fs(&dyn FileSystemProvider)` 返 `Arc<Self>`，内部 `LazyLock<Arc<BackendResolvers>>` 静态实例缓存避免每次构造时重创建 trait object 与 SearchConfig

```rust
// cdt-fs/src/backend_policy.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPolicy {
    // existing 4
    pub initial_load_policy: InitialLoadPolicy,
    pub max_round_trips_for_initial_page: u8,
    pub supports_incremental_updates: bool,
    pub prefetch_policy: PrefetchPolicy,
    // PR-E new 3
    pub supports_memory: bool,
    pub supports_subagent_scan: bool,
    pub stale_check_strategy: StaleCheckStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleCheckStrategy {
    /// 本机 mtime 与 `SystemTime::now()` 比 5min 阈值；超时视为 crashed/killed。
    LocalClock5min,
    /// 远端 mtime 与本机 clock 跨 domain（远端时钟回拨/时差）— 5min 阈值不可比对，
    /// 跳过 stale check 避免 false positive/negative。
    SkipUntilClockSync,
}

impl BackendPolicy {
    pub const fn for_local() -> Self { /* +supports_memory: true, +supports_subagent_scan: true, +stale_check_strategy: LocalClock5min */ }
    pub const fn for_ssh() -> Self { /* +supports_memory: false, +supports_subagent_scan: false, +stale_check_strategy: SkipUntilClockSync */ }
    pub const fn for_http() -> Self { /* 按 Local 兜底：HTTP 当前用 Local 数据源，行为与 Local 一致 */ }
}
```

```rust
// cdt-api/src/ipc/backend_resolvers.rs
use std::sync::{Arc, LazyLock};
use cdt_discover::{FileSystemProvider, FsKind, GitIdentityResolver,
                   LocalGitIdentityResolver, SearchConfig};

pub(crate) struct BackendResolvers {
    pub search_config: SearchConfig,
    pub git_identity_resolver: Arc<dyn GitIdentityResolver>,
}

static LOCAL_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| {
    Arc::new(BackendResolvers {
        search_config: SearchConfig::default(),
        git_identity_resolver: Arc::new(LocalGitIdentityResolver::new()),
    })
});

static SSH_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| {
    Arc::new(BackendResolvers {
        search_config: SearchConfig { is_ssh: true, ..SearchConfig::default() },
        git_identity_resolver: Arc::new(NoopGitIdentityResolver),
    })
});

impl BackendResolvers {
    pub fn for_local() -> Arc<Self> { LOCAL_RESOLVERS.clone() }
    pub fn for_ssh() -> Arc<Self> { SSH_RESOLVERS.clone() }
    pub fn from_fs(fs: &dyn FileSystemProvider) -> Arc<Self> {
        match fs.kind() {
            FsKind::Local => Self::for_local(),
            FsKind::Ssh => Self::for_ssh(),
        }
    }
}

/// SSH context 下取代 `LocalGitIdentityResolver`：远端 .git 不可访问（不能 spawn
/// 子进程，多数远端是非 git 项目），所有 git 字段返回 None / true 兜底。从
/// `local.rs::NoopGitIdentityResolver` 抽到本 module（与 BackendResolvers 同生命周期）。
struct NoopGitIdentityResolver;
#[async_trait::async_trait]
impl GitIdentityResolver for NoopGitIdentityResolver { /* 同现状三个方法 */ }
```

**为何不直接合一个 struct**：
- 合一 → cdt-fs 依赖 cdt-discover/cdt-core → 破 fs-abstraction H1 / Scenario "cdt-fs 不依赖业务 crate"
- 拆 → cdt-fs 保持纯净（基础设施层），cdt-api 持 business resolvers（业务层），调用方一次 `active_fs_and_policy()` 拿俩，使用感无差

**为何不把 `GitIdentityResolver` trait 与 `SearchConfig` 下沉到 cdt-fs**：
- `GitIdentityResolver` 依赖 `cdt-core::RepositoryIdentity`；cdt-fs 也不能依赖 cdt-core
- `SearchConfig` 字段简单（4 个 primitive）可以下沉但 `SessionSearcher` 用它且 SessionSearcher 在 cdt-discover；下沉 SearchConfig 会让 cdt-discover 反向 import cdt-fs 的 SearchConfig（导致 type 飘）—— 不如保留原位
- 沉淀工程价值不如承担 cdt-api 内一个小 module，且 BackendResolvers 本质就是"业务策略落点"——放业务 crate 更对位

**替代方案**：
- (a) BackendPolicy 一体化（cdt-fs 加 cdt-discover 依赖）→ 否决（破 spec Scenario "cdt-fs 不依赖业务 crate"）
- (b) 把 GitIdentityResolver + SearchConfig 下沉到 cdt-fs → 否决（GitIdentityResolver 依赖 cdt-core，下沉链路过长）
- (c) BackendPolicy 留 primitive，resolvers 分到 cdt-api（本节版本）→ 选中
- (d) BackendPolicy 用 generic 参数 `BackendPolicy<R: GitIdentityResolver>` → 否决（generic 化导致 LocalDataApi 字段变 generic，ripple 全 crate；本仓只 Local + Ssh 二元用 generic 是 overkill）

### D2: `stale_check_strategy` 用 enum 而非 fn pointer / `Box<dyn Fn>`

**问题**：line 2334 stale check 在 SSH 时跳过；未来可能引入 SSH-aware clock skew compensation（用远端 EPOCHSECONDS 测 offset 后做 5min 比对）。这意味着策略可能从 2 增到 3+。是用 enum 还是函数指针？

**修法**：选 `enum StaleCheckStrategy { LocalClock5min, SkipUntilClockSync }`。

理由：
- 本仓只 2 backend (Local + Ssh)；策略 2 个 variant 已够覆盖；fn pointer / `Box<dyn Fn>` 提供的灵活性溢出
- enum 让 `BackendPolicy` 保留 `Copy + Eq`；fn pointer 也能（fn pointer 是 Copy）但 `Box<dyn Fn>` 不行
- 调用方用 `match` 派发，比函数调用更直观，且当未来加 `ClockSkewCompensated { offset_secs: i64 }` variant 时所有 callsite 编译期强制处理（exhaustive match），fn pointer 不强制
- 字段类型出现在 spec scenario 里，enum variant 名直接是 self-documenting

**未来扩展点**：加新 variant `ClockSkewCompensated { offset_secs: i64 }` 时 BackendPolicy 仍 Copy + Eq（i64 是 Copy）；调用方 match 一处一处处理。

**替代方案**：
- (a) fn pointer `fn(SystemTime, SystemTime) -> bool` → 否决（语义不强，缺乏可读 variant 名；exhaustive match 缺失）
- (b) `Box<dyn Fn>` → 否决（破 Copy，序列化困难，单测 Eq 难写）
- (c) enum（本节）→ 选中

### D3: `policy.supports_memory == false` 时 `read_memory_file` 仍返 not_found（不抛 contract violation）

**问题**：5 个 codex 重点问题之一——前端可能在不知 BackendPolicy 状态下调 `read_memory_file`，后端应：(a) 拒绝调用（panic / 强 contract violation），还是 (b) 返普通 `not_found` 让前端按"该文件不存在"显示？

**修法**：选 (b) —— 返 `ApiError::not_found(...)`，错误消息显式提及"SSH context 下远端 memory 文件读取尚不支持"。

理由：
- "API 契约层 vs 实现层"：契约层（IPC method 仍可调，前端不需 BackendPolicy 状态感知）是健壮设计；实现层根据 policy 选择行为（返 empty / not_found）
- 当前 line 2240-2243 已是这个行为（返 `not_found` 带 SSH 提示），本 PR 仅把 `if fs.kind() == Ssh` 改成 `if !policy.supports_memory`，**不**改契约
- 未来若加 `supports_memory_partial: bool` 等更细策略，仍只是字段值变化，不破契约
- 前端 UI i18n 文案改进（区分 "not found" vs "not supported on SSH"）留 follow-up（PR-G）

**`get_project_memory` 返空 `ProjectMemory` vs 抛错**：保留现状返空 + `has_memory: false` —— 与"列表查询返空结果"语义一致，比抛错更友好（UI 显示"无 memory 内容"而非 error toast）。

**替代方案**：
- (a) Contract violation（panic / 显式 unsupported error code）→ 否决（前端要先查 backend policy 才能调，复杂度暴涨）
- (b) 返 not_found 保留现状语义（本节）→ 选中

### D4: `BackendResolvers` 用 `LazyLock<Arc<Self>>` 静态实例避免每次构造

**问题**：6 处 callsite 都在 IPC handler 内每次调用——若每次都 `BackendResolvers { search_config: ..., git_identity_resolver: Arc::new(LocalGitIdentityResolver::new()) }` 重创建：
- `Arc::new(LocalGitIdentityResolver::new())` 每次 heap alloc + Arc 计数（虽然结构是 unit struct 但 Arc<dyn> 仍 alloc 一次 vtable+counter）
- 高频 IPC（list_repository_groups 在 sidebar 渲染时高频触发）下累积无谓 allocation

**修法**：用 `std::sync::LazyLock<Arc<BackendResolvers>>` 静态缓存，`for_local()` / `for_ssh()` 返 `Arc::clone(&STATIC)`（仅 atomic counter inc，零 alloc）。

```rust
static LOCAL_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| { ... });
static SSH_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| { ... });
```

`SearchConfig` 字段在 LazyLock 构造时一次完成，调用方 `resolvers.search_config.clone()` 仍是 `Vec::clone()`（4 element），可忽略。或更进一步 `&resolvers.search_config` 借用免 clone（看 SessionSearcher signature 选择）。

**为何 `LazyLock` 不 `OnceCell` / `OnceLock`**：`LazyLock` 是 std 1.80+ 标准库 API（本仓 rust-toolchain stable），用法最简洁。`OnceLock` + 手写 init 闭包也 OK 但 boilerplate 多。

**替代方案**：
- (a) 每次新建 → 否决（无谓 alloc）
- (b) `Arc<BackendResolvers>` 作 LocalDataApi 字段（new 时初始化）→ 否决（要在 LocalDataApi 内持俩字段 local/ssh，且 active context 切换时需要逻辑选择；不如全局 LazyLock 简洁）
- (c) `LazyLock`（本节）→ 选中

### D5: `active_fs_and_policy()` helper 返五元组，与 `active_fs_and_context_strict()` 同语义快照

**问题**：6 处 callsite 之前都 `let (fs, projects_dir) = active_fs_and_projects_dir().await?` + `let is_remote = fs.kind() == Ssh`。本 PR 后要拿 `fs + projects_dir + ctx + policy + resolvers`。每处都重复 5 次 await + 派生不雅；且 SSH disconnect 中间态 race 需要五元组同快照。

**修法**：加 `active_fs_and_policy()` helper：

```rust
pub(crate) async fn active_fs_and_policy(
    &self,
) -> Result<
    (
        Arc<dyn FileSystemProvider>,
        PathBuf,
        cdt_fs::ContextId,
        Arc<BackendPolicy>,  // 或 BackendPolicy by-value (Copy)
        Arc<BackendResolvers>,
    ),
    ApiError,
> {
    let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
    let policy = Arc::new(match fs.kind() {
        FsKind::Local => BackendPolicy::for_local(),
        FsKind::Ssh => BackendPolicy::for_ssh(),
    });
    let resolvers = BackendResolvers::from_fs(&*fs);
    Ok((fs, projects_dir, ctx, policy, resolvers))
}
```

**关于 `Arc<BackendPolicy>` vs by-value `BackendPolicy`**：BackendPolicy 是 Copy（所有字段 primitive），by-value 更省一次 Arc 计数；但 caller pattern 拆 5 元组时 `Arc<>` 与 by-value 在用法上等价（都用 `policy.supports_memory`）。本 PR 选 **by-value**（不包 Arc）—— Copy 类型包 Arc 是反 idiom；BackendResolvers 保留 Arc 因为它持 trait object 必须 indirect。

**统一改五元组 vs 加另一个 helper**：5 元组在 Rust 里偏长但 readable（同 `active_fs_and_context_strict` 三元组的扩展）；分两个 helper 调用方仍要 2 次 await + 2 处 race window。一次 helper 一次 await 最简。

**所有 6 处 callsite 都改成调 `active_fs_and_policy`？**：是的——读 unused 字段不破任何东西（policy / resolvers 都 cheap）。这统一了"任何 IPC handler 拿 fs/ctx 第一步是 `active_fs_and_policy`"的代码规范。

**例外**：`get_project_memory` / `read_memory_file` 当前只调 `active_fs_and_projects_dir`（旧 helper 不带 ctx），本 PR 改它们调 `active_fs_and_policy` —— ctx 不用但拿到无害；policy.supports_memory 是关键字段。其它仅需 fs+projects_dir 的 IPC handler（非 6 处之一）保留旧 helper 不变 —— 本 PR 不收口非相关 callsite。

**替代方案**：
- (a) 每 callsite 各自取 fs + 派生 policy → 否决（5 处重复模板）
- (b) 仅加 `BackendPolicy + BackendResolvers` 派生方法（无 helper） → 否决（仍要每处先调 active_fs_and_context_strict）
- (c) 五元组 helper（本节）→ 选中

### D6: grep 不变性测试拦截"未来加新的 fs.kind() 分叉"

**问题**：本 PR 改完 6 处分叉但后续 PR 可能再加新 `if fs.kind() == Ssh` —— 怎么防回潮？

**修法**：加 `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 集成测试：
- 用 `std::fs::read_to_string` 读 `crates/cdt-api/src/ipc/local.rs`
- 计算两种 pattern 各自出现次数：(a) `fs.kind() ==`；(b) `let is_remote =`
- 断言 ≤ 真实剩余阈值（codex design 二审 Blocking #1 修订）

**真实阈值**（grep main HEAD `5d0207a` 后逐行确认）：
- PR-E 后剩余 `let is_remote =` **2 处**：837（`list_sessions_skeleton` SSH page cache lookup 派生）、1594（`build_group_session_page` 同理）—— 这俩属 PR-D D2 落地的 SSH-Local 同入口下的内部派生，**不在 PR-E 6 处之列**
- PR-E 后剩余 `fs.kind() ==` **3 处**：837 与 1594（作为 `let is_remote = fs.kind() == ...` 的右值出现）、3125（`read_mentioned_file` SSH gate；codex 二审 Open Question 标 follow-up）

测试硬编码两阈值（`MAX_LET_IS_REMOTE = 2` / `MAX_FS_KIND_EQ = 3`）+ 顶部注释列每行剩余出处与合理性引用（PR-D D2 / Open Question）。阈值超 → fail；新增 fork 的 PR 必须显式调整阈值 + 在 PR 描述说明合理性，避免静默回潮。

**测试覆盖范围**：仅 `crates/cdt-api/src/ipc/local.rs`（不扫整个 cdt-api，避免误报 BackendResolvers / 其它模块构造时的合理使用）；BackendResolvers 在 backend_resolvers.rs 内部 `from_fs(&fs)` 用 `fs.kind()` 派发——这是 D1 钉死的"派生点"，本测试不扫该文件。

**为何不 xtask**：xtask 适合 workspace-wide 静态扫；本约束仅针对 local.rs 单文件 + 阈值含语义判断，集成测试 + 硬编码注释更适合。未来若扩到多文件可升级为 xtask。

**替代方案**：
- (a) 不加测 → 否决（手 review 易漏，policy fork 注释也不能保证未来 PR 遵守）
- (b) clippy custom lint → 否决（learning curve 高，本约束太具体）
- (c) xtask 全 workspace 扫 → 否决（误报多，调用 ALLOWLIST 复杂度）
- (d) 单文件 grep 集成测试 + 硬编码阈值注释（本节）→ 选中

### D7: `WorktreeGrouper::new_dyn(Arc<dyn GitIdentityResolver>)` thin wrapper 入口

**问题**：现有 `WorktreeGrouper::new<R: GitIdentityResolver>(resolver: R)` 是 generic，BackendResolvers 持 `Arc<dyn GitIdentityResolver>` 无法直接传入。

**修法（codex design 二审 Blocking #2 修订）**：`async-trait` macro **不会**自动为 `Arc<T: Trait + ?Sized>` 生成 trait blanket impl —— design 与 tasks SHALL **显式**要求添加该 blanket impl，**不能**假定 "should just work"。

具体两步：

**Step A**：`crates/cdt-discover/src/worktree_grouper.rs` 显式加 blanket impl：

```rust
#[async_trait]
impl<T: GitIdentityResolver + ?Sized + Send + Sync> GitIdentityResolver for Arc<T> {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity> {
        (**self).resolve_identity(path).await
    }
    async fn get_branch(&self, path: &Path) -> Option<String> {
        (**self).get_branch(path).await
    }
    async fn is_main_worktree(&self, path: &Path) -> bool {
        (**self).is_main_worktree(path).await
    }
    async fn resolve_all(&self, path: &Path) -> RepoLookup {
        (**self).resolve_all(path).await
    }
}
```

**Step B**：加 `WorktreeGrouper::new_dyn(resolver: Arc<dyn GitIdentityResolver>) -> WorktreeGrouper<Arc<dyn GitIdentityResolver>>` 入口（复用 generic `new`，依赖 Step A blanket impl）：

```rust
impl WorktreeGrouper<Arc<dyn GitIdentityResolver>> {
    pub fn new_dyn(resolver: Arc<dyn GitIdentityResolver>) -> Self {
        Self::new(resolver)
    }
}
```

**Fallback**：若 Step A blanket impl 因 async-trait macro 边界条件（如返回类型含 generic associated lifetime）编译失败，改走 newtype wrapper：

```rust
struct DynGitIdentityResolver(Arc<dyn GitIdentityResolver>);
#[async_trait]
impl GitIdentityResolver for DynGitIdentityResolver {
    // 同上四方法 forward 到 self.0
}
```

tasks 阶段先按 Step A blanket impl 实施；编译失败时切 fallback newtype；两条路径都跑 `cargo test -p cdt-discover` 验证。

**spec 影响**：`WorktreeGrouper::new` 现有 generic API 保留；`new_dyn` 是新 API；blanket impl 为 `Arc<dyn GitIdentityResolver>` 提供 trait conformance，**不破**现有 caller。

**替代方案**：
- (a) 改 `WorktreeGrouper::new` 签名为 `Arc<dyn GitIdentityResolver>` → 否决（破现有 generic + 测试用 `WorktreeGrouper::new(LocalGitIdentityResolver)` 等单测）
- (b) `BackendResolvers` 不持 Arc<dyn> 改持泛型 → 否决（破 BackendResolvers 单一类型 + LazyLock 静态实例）
- (c) thin wrapper `new_dyn` + 显式 blanket impl / fallback newtype（本节版本）→ 选中

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| `BackendPolicy` 加字段破现有 4 个测试（assert exhaustive 等）| 单测全部跟新（仅 backend_policy.rs 一个文件）；现有 `for_local / for_ssh / for_http` 单测加新字段断言；`prefetch_and_initial_load_are_orthogonal` 测试不动 |
| `BackendPolicy::for_http()` 应取 Local 还是 SSH 字段值 | 选 Local（HTTP backend 当前用 Local 数据源，行为与 Local 一致；spec 现状即如此）；加单测覆盖每字段 |
| 6 处 callsite 改完后行为微妙退化（如 stale check enum match 写错）| 每 callsite 改后跑 `cargo test --workspace`；ipc_contract 测保护字段格式；手动跑 `just dev` smoke Local + SSH 列表 |
| `Arc<dyn GitIdentityResolver>` 自动 trait forward 不工作 | 编译期发现；fallback 加 newtype wrapper 实现 trait |
| grep 不变性测试阈值过紧导致正常重构难推进 | 阈值含注释说明每处出处；新增 fork 时调阈值 + PR 说明；不强制 0 |
| 5 处 callsite + 改 helper 函数签名导致集成测试破坏 | crate-private helper，调用方都在 local.rs 内部；ipc_contract 测无变化（公开 IPC 字段不变）|
| BackendResolvers 在 cdt-api 而非 cdt-fs，未来 cdt-cli / cdt-http 想用怎么办 | 当前只有 cdt-api 持 LocalDataApi 用；cdt-cli 直接通过 cdt-api 公开方法；未来若 cdt-cli 直访 BackendResolvers 再上移 cdt-discover/cdt-core（届时也未必出现）|
| `Arc<BackendResolvers>` 通过 LazyLock 静态缓存——单测改 SearchConfig 默认值时缓存污染？ | LazyLock 在测试 binary 内独立实例（test binary 独立 process，每次 cargo test 重新 init）；同 binary 多个 #[test] 都共享同份默认值无问题 |
| codex 报新 design 问题 | 本 design 进 codex 二审，按反馈修订；apply 阶段每次 push 后再调 codex |
| Perf 回归（5 元组 helper 多 await + Arc clone overhead）| `bash scripts/run-perf-bench.sh --runs 5` 四维 verify；Arc::clone 是单 atomic increment，纳秒级 |

## Migration Plan

本 change 是基建清理 + 架构上移，对前端 IPC 无 BREAKING，对外部测试无 BREAKING（公开 trait / 公开 type 签名都保留或加 sibling 入口）。

**部署顺序**（apply 阶段建议）：

1. cdt-fs：扩 `BackendPolicy` 加 3 字段 + `StaleCheckStrategy` enum；更新 `for_local` / `for_ssh` / `for_http` const 构造器；更新单测（每字段断言）
2. cdt-discover：加 `WorktreeGrouper::new_dyn(Arc<dyn GitIdentityResolver>)` thin wrapper（若 async-trait 自动 forward 不工作，加 newtype）+ 单测覆盖
3. cdt-api：新增 `crates/cdt-api/src/ipc/backend_resolvers.rs` module + `BackendResolvers` + `NoopGitIdentityResolver`（从 local.rs 抽过来）+ `LazyLock` 静态实例 + 单测
4. cdt-api：在 LocalDataApi 加 `active_fs_and_policy()` helper（基于现有 `active_fs_and_context_strict`）
5. cdt-api：改 6 处 callsite（local.rs）逐一替换 `if fs.kind() == Ssh` → policy/resolvers 字段读取
6. cdt-api：删 local.rs 内的 inline `NoopGitIdentityResolver`（已搬到 backend_resolvers.rs）
7. cdt-api：加 `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 不变性测试
8. spec delta + validate：`openspec/changes/backend-policy-struct/specs/fs-abstraction/spec.md` MODIFY `BackendPolicy enum 雏形定义` Requirement
9. perf 校验：apply 前后跑 `bash scripts/run-perf-bench.sh --runs 5`
10. codex 二审 push 前 + push 后多轮

**回滚**：本 change 改动隔离在 cdt-fs/backend_policy.rs + cdt-api/ipc/backend_resolvers.rs + cdt-api/ipc/local.rs + cdt-discover/worktree_grouper.rs + 一个新测试文件；revert PR 即可。无数据迁移、无前端联动 BREAKING。

## Open Questions

1. **`BackendPolicy::for_http()` 是否真等同 `for_local()`？** —— HTTP backend 当前在 server mode 把 LocalDataApi 复用为数据源（即 HTTP server 跑在 Local 上访问 Local `~/.claude/`），所以行为与 Local 一致。未来若 HTTP server 接 SSH backend 再加分支。本 PR 选 for_http = for_local。

2. **是否在 `active_fs_and_policy()` 返 `BackendPolicy` by-value 还是 `Arc<BackendPolicy>`？** —— 当前选 by-value（Copy 类型包 Arc 是反 idiom）；BackendResolvers 仍 Arc。若 codex 提"统一用 Arc 简化 cache 语义"再调整。

3. **`StaleCheckStrategy` 是否需要 `ClockSkewCompensated { offset_secs: i64 }` variant 占位？** —— 本 PR 仅落 2 个 variant；占位 variant 增加未实现路径让 reviewer 困惑。留 PR-G 真实施再加。

4. **是否在 spec delta 里写"业务代码 SHALL NOT 直接判 fs.kind()" 的 SHALL 句？** —— 偏严格但更对位 H3 钉死的"业务算法代码 fs.kind() == Ssh 默认拒"。本 PR 选写 SHALL 句 + 引 D6 不变性测试 scenario 兜底。

5. **`BackendResolvers` 字段是 `pub` 还是 `pub(crate)` + accessor?** —— 本 PR 选 `pub`（在 module 内）+ struct `pub(crate)`（不暴露给 crate 外）—— 调用方 `resolvers.search_config` / `resolvers.git_identity_resolver.clone()` 直读，无需 getter。

6. **`read_mentioned_file` SSH gate (line 3125) 是否也归 PR-E 范围？** —— codex design 二审 Open Question。当前不在用户列的 6 处分叉之内（无 `// policy fork: PR-E lift` 注释），但本质上是同款"业务代码直接判 fs.kind() == Ssh"反模式。本 PR **不**收口（保持 scope 边界清晰、与用户输入一致），但在 D6 grep 阈值中显式承认其存在 + 留 follow-up（典型字段名 `supports_mention_file_resolution: bool` 或 `mention_resolver: Arc<dyn MentionResolver>` 走 BackendResolvers）。

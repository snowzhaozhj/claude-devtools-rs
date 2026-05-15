# 性能基线 + 预算 + 防回归

claude-devtools-rs 用 Rust 重写原 TS 项目的根本动机就是性能。本文是**硬约束**——任何 PR 都要参照此规则做性能影响评估。

## 性能预算（关键路径）

这些是冷启动 / 首屏 / 大会话三条关键路径的上限。**违反需要 follow-up 修复**，不能合并 PR 时降低预算。

| 关键路径 | 测量方式 | 预算 | 基线（v0.4.8，2026-05-15，27 project × 534 session）|
|---|---|---|---|
| **冷启动 list_repository_groups** | `cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture`，看 `cold total` | **< 200ms** | scan=87ms + grouper=2ms = **89ms** |
| **冷启动 list_projects** | 上同，看 `cold scan` | **< 150ms** | 87ms |
| **首屏 sidebar 可见列表** | 桌面应用启动到 sidebar 首条 session 渲染（人工秒表） | **< 500ms** | 待测 |
| **大会话 get_session_detail** | `cargo test -p cdt-api --release --test perf_get_session_detail -- --ignored --nocapture`，看各阶段 + payload | 各阶段总 **< 800ms（10k 消息会话）** | 详见 perf-bench skill 输出 |
| **Tauri IPC payload** | 后端 emit JSON size | 单次返回 **< 1 MB**（>1MB 须走瘦身模式） | — |
| **Tauri IPC 端到端吞吐** | 实测 webview 端 JSON.parse 完成 | ≈ **6.5 KB/ms**（含反序列化） | — |

**计算逻辑**：基线允许的回归 **≤ 20%**。超出 20% 视为性能 bug，PR 不合并。

## 现有 perf bench 入口

每个关键路径都有 bench tool（`#[ignore]` 不进 CI，手动跑作为定位 + 基线对比）：

- `cdt-api/tests/perf_cold_scan.rs::measure_cold_scan` — 冷启动 scan + grouper 链路
- `cdt-api/tests/perf_get_session_detail.rs::measure_get_session_detail` — 大会话首次打开
- `/perf-bench` skill — 自动跑 + 解析 + 给 "瘦身/不瘦身" verdict

**新增关键路径时**：SHALL 加对应 bench（参照上述模板）+ 把基线数据填进本文预算表。

## 防回归硬约束

### 1. PR 影响评估（强制）

涉及以下任一文件 / 行为的 PR，**SHALL** 在 PR 描述里加 "Perf impact" 段并跑相应 bench 给数据：

- `cdt-discover/` 任何文件（启动 + sidebar 列表必经路径）
- `cdt-api/src/ipc/local.rs` / `session_metadata.rs` / `cache_signature.rs`（IPC 数据流核心）
- `cdt-analyze/` 任何 `build_chunks` / context tracking 路径
- 引入 `tokio::process::Command` / `Command::new` 子进程 spawn（成本极高）
- 在 hot loop 里加 `tokio::fs::read_to_string` 全文件读 / `JSON.parse` 大对象
- 改 `tauri.conf.json` / `src-tauri/Cargo.toml` features（影响 bundle / startup binary size）

模板：
```markdown
## Perf impact
- 关键路径：[冷启动 / get_session_detail / ...]
- 基线：xxx ms
- 本 PR 后：yyy ms（±zz%）
- 数据来源：`cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture` 输出
```

豁免：纯 docs / 注释 / typo / CI 配置改动可不写。

### 2. 反模式清单（**严禁**引入）

历史血泪经验，违反任一即拒：

- **for-loop 内串行 spawn 子进程**：spawn 单次 cold 5–15ms，N 个串行就是 N×。如果真要 spawn N 个用 `futures::future::join_all` 并发，且优先看能不能换纯 fs / lib 调用
- **for-loop 内串行 file I/O**：`for x { tokio::fs::read_to_string(x).await }` 等价上一条。用 `join_all` 并发 + 加 `Semaphore` 限流
- **每次 IPC 都重扫文件 / 重算 chunk**：必须有按 `FileSignature` 的内存 cache（参照 `MetadataCache`）
- **冷启动路径同步阻塞 I/O**：`std::fs::*` / `Command::output().wait()` 都会阻塞 tokio worker；用 `tokio::fs::*` / `Command::output().await`
- **每次返回全量 JSON 而不分页 / lazy**：>1 MB payload SHALL 走"IPC payload 瘦身模式"（参照 CLAUDE.md Conventions）
- **`spawn` 创建新 tokio runtime 当作并发原语**：runtime 是重对象，PR 评审看到当 bug
- **不走 cache fast-path 的 IPC**：详见 `ipc-data-api/spec.md::list_sessions cache 命中`
- **算法 O(N²) 在 N > 100 时**：list / merge / sort 都要看复杂度——本仓 27 project × 534 session 不算大

### 3. 新功能性能验收

任何**新增 capability / 改后端算法 / 加 IPC 字段** 的 PR：
- SHALL 在 spec 里加一条 **性能 SHALL**（如 "list_xxx 在 N=500 时 SHALL < 100ms"）
- SHALL 加对应 bench 覆盖该 SHALL
- SHALL 在 PR 描述贴 bench 输出

事后补不算——和 "openspec 行为契约级改动先 propose 再 apply" 同样原则。

### 4. codex 二审增加性能视角

`.claude/rules/codex-usage.md` 已规定每个 PR push 后默认调 codex。**性能相关 PR** 的 codex prompt SHALL 显式列：

```
重点查：
- 是否有 for-loop 内 spawn / 串行 await（应该 join_all）
- 是否有 hot path 缺 cache（按 FileSignature key）
- 是否有重复 IPC payload 字段（应该 omit 或 lazy）
- 算法复杂度评估
```

## 主动定期跑

**每发版前** SHALL 跑以下 bench 并对比上一次基线：

```sh
# 冷启动
cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture

# 大会话
cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
```

把数据贴到 release PR 描述，方便回溯。

**每次会话开始 + 用户问 "为什么慢"** SHALL 先跑 bench 拿数据再讨论方向，不靠直觉。

## 历史性能事件（学习材料）

按时间倒序：

- **2026-05-15** `perf/cold-start-list-sessions` — list_repository_groups 4030ms → 89ms（45x）
  - 根因：`LocalGitIdentityResolver` 每 project 串行 spawn 3–5 个 `git rev-parse` 子进程
  - 修法：纯 fs 实现，从 `.git` / `HEAD` 文件直接读取，0 个 git 子进程
  - 教训：**有文件可读时绝不 spawn 子进程**——syscall 比 process spawn 快 1000 倍

- **2026-05-14** `multi-session-cpu-cache` + `session-list-cache-fast-path` — list_sessions 全 cache 命中路径
  - 根因：每次 IPC 全部 session 都重扫 JSONL
  - 修法：按 `FileSignature` LRU cache + fast-path 跳过 broadcast 路径

- **2026-04-19** `session-detail-image-asset-cache` — 大会话 image 反复 base64
  - 根因：每次 `get_session_detail` 把所有 image block base64 inline
  - 修法：image 落盘 cache + 走 `asset://` URL

- **2026-04-29 ~ 05-12** 5 轮 IPC payload 瘦身（详见 `feedback_align_with_original.md` 上下文）
  - 根因：default-cap / response content 全量塞 IPC
  - 修法：`OMIT_XXX const + xxxOmitted: bool + get_xxx_lazy IPC` 模式

## 后续性能优化候选清单

按收益排序，开 follow-up issue 或 openspec change 处理：

1. **scan() 顶层 project 目录并发**：当前 `for dir_name in dirs` 顺序处理 27 dir，可改 `join_all` 并发——预期 scan 87ms → ~30ms
2. **head N 行 cwd 抽取的 fallback `read_to_string` 全文件去掉**：大会话 fixture 卡几十 ms，spec 没强制 Local 也要 fallback（SSH 已禁用）
3. **持久化 cwd / git 元数据 cache**：跨进程 cache 让"冷冷启动"也命中（首次 install 后第二次启动直接零扫）
4. **sidebar 列表渲染 `{#each}` 的虚拟滚动 lazy mount**：列表项 > 200 时有微卡顿（现实只有 500 也很流畅，但需要 budget 限定）
5. **chunk-building 复杂度 audit**：`build_chunks_with_subagents` 对 10k 消息会话耗时是否 O(N²)？需 bench 验
6. **WorktreeGrouper 进程内 cache**：当前实现已经很快（2ms），但同进程多次调（如 sidebar refresh）每次重扫——加 in-memory cache 完全避免重复

每条候选都 SHALL 走 openspec propose + 加 bench 验证 + 给数据。

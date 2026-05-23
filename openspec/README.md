# OpenSpec in claude-devtools-rs

This directory mirrors the spec baseline established in the parent
TypeScript project (`../claude-devtools`) on bootstrap day and is now
**owned independently** by this Rust repo. Future changes happen here
first; the TS repo is frozen for reference purposes.

## Layout

```
openspec/
├── config.yaml                  # Schema = spec-driven, project context
├── specs/                       # Capability specs (authoritative behavioral contract)
├── changes/                     # Active and archived changes for the Rust port
└── TS_BASELINE_DEVIATIONS.md    # TS port 偏差预警 + UI 隐式契约（main 既有 bug 走 GitHub Issue）
```

## Source of truth

- `specs/` is the authoritative behavioral contract for the Rust port.
- `TS_BASELINE_DEVIATIONS.md` lists places where the **TypeScript implementation deviated
  from its own spec** (port 期发现的有意偏离 / UI 隐式契约). When porting, implement the spec — do **not** copy the
  TS bug.
- **main 上的既有 bug / coverage gap / 跨 capability 长期跟踪** → 走 GitHub Issue（默认 `bug` label）。归宿规则详根 `CLAUDE.md::遗留事项归宿`。

## Workflow

1. Use `/opsx:propose <name>` for a new port change (one capability per change is ideal)
2. Use `/opsx:apply` to work through tasks
3. Use `/opsx:archive` when a change is merged and its delta should move into `specs/`

## Capability → crate map

| Capability                     | Owning crate    |
|--------------------------------|-----------------|
| project-discovery              | `cdt-discover`  |
| session-parsing                | `cdt-parse`     |
| chunk-building                 | `cdt-analyze`   |
| tool-execution-linking         | `cdt-analyze`   |
| context-tracking               | `cdt-analyze`   |
| team-coordination-metadata     | `cdt-analyze`   |
| session-search                 | `cdt-discover`  |
| file-watching                  | `cdt-watch`     |
| configuration-management       | `cdt-config`    |
| notification-triggers          | `cdt-config`    |
| ssh-remote-context             | `cdt-ssh`       |
| ipc-data-api                   | `cdt-api`       |
| http-data-api                  | `cdt-api`       |

## 路线图

下面是已识别但**未承诺时间**的未来 change 候选——观察真实数据 / 用户反馈后再决定优先级。具体可执行的 backlog 走 GitHub Issues（含 milestone）。

### cdt-telemetry Phase 2/3/4 候选 slug

设计阶段固化（change `add-telemetry-signal-bus` 的 `design.md::Migration Plan`）：

| Phase | slug 候选 | 触发条件 | 主要内容 |
|---|---|---|---|
| 2 | `add-telemetry-persistence` | Phase 1 上线 1 周 + 监控 telemetry 自身无回归 | SQLite 持久 + 90 天 retention + 历史趋势 UI |
| 3 | `add-telemetry-resource-and-ci` | Phase 2 上线 + 用户反馈"想看资源占用" | Resource 维度 sampler（RSS / FD / tokio task）+ tracing bridge 方向 2 digest + CI bench 集成 telemetry snapshot |
| 4 | `add-telemetry-opt-in-reporting` | 隐私合规 review 完成 + 后端聚合 endpoint | opt-in 上报通道 + Behavior 维度（项目路径 hash / IPC 调用频次）+ 隐私 review pass |

后续 Phase change `MUST` 在 proposal.md 显式引用本设计的 Phase 划分表，验证：Phase 2 SHALL 复用 `TelemetrySnapshot` schema；Phase 3 资源 sampler SHALL 接入既有 Registry；Phase 4 上报路径 SHALL 在信号上加 `reportable: bool` 标签 + 黑名单（path / sessionId / backtrace）不出本机。

### Phase 1.5 micro-task 候选

观察 24h-1 week 真实数据后再决定是否做：

- **信号爆炸退避策略**：tracing layer ERROR/WARN 风暴时（> 1000 / 秒）启用指数退避采样，避免 cdt_xxx.error counter 爆涨虚报频率
- **tracing target 子模块归类细分**：当前按顶级 crate 归类（`cdt_ssh.error`），观察 cdt-ssh 多模块（manager / polling_watcher / session）失败模式是否需要细分到 `cdt_ssh.polling.error` 等
- **Event ring buffer 利用率监控**：`events.dropped` counter 与队列 cap 10000 是否合适

### Phase 1.5 deferred 三件（Phase 1 PR 之外的衍生）

由后续独立 PR 跟进，不阻塞 Phase 1 验收：

- **3.4 ssh.reconnect / 3.5 ssh.sftp_death**：`cdt-ssh::SshConnectionManager` 当前不区分"首次 connect vs 自愈 reconnect"路径。需先在 cdt-ssh 增加 `reconnect: bool` 语义标记（或 `connection_attempt_count` / `last_disconnect_at`）才能正确 inc。registry 内 counter 已注册占位
- **3.6 watcher.respawn**：`cdt-watch` 当前不显式 respawn watcher（依赖 OS notify backend 重连）。`watcher.respawn` counter 已注册占位；待 cdt-watch 加显式自愈逻辑（如 backend reload）时同步接入
- **perf_telemetry_overhead 专项测试**：需要稳定的 try_lookup_cached_metadata fixture（多 ContextId × cache miss/hit 分支），独立 follow-up PR 写更聚焦
- **DiagnosticsTab.test.svelte.ts 组件测试**：vitest 单测 + mockIPC 路径与 Sidebar.test.svelte.ts 同 pattern，独立 follow-up PR

### 已生效但 deferred 的 reliability 信号

tracing layer 自动归类 `cdt_ssh.error` / `cdt_ssh.warn` / `cdt_watch.error` / `cdt_watch.warn`——**4 路 ERROR/WARN 频次**已由现有 `tracing::error!` / `tracing::warn!` 调用自动采集，无侵入。SSH self-heal / watcher 假死的初步 surface 已转白盒。

---

## Migration history

- **2026-05-23**：原 `followups.md`（712 行混合 TS 偏差预警 / main bug / 已修索引 / 路线图）改造为 `TS_BASELINE_DEVIATIONS.md`（仅 TS 偏差 + UI 隐式契约 + 少量 backlog）。main 既有 bug 迁出到 GitHub Issues #230-#239；路线图候选挪到本文件。

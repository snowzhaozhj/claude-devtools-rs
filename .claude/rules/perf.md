# 性能基线 + 预算 + 防回归

claude-devtools-rs 是**桌面辅助工具**，不是用户主线（IDE / 浏览器 / 编辑器）。开着不应感知它存在：风扇不起转、电池不掉、不抢其他 app 的 CPU。本文是**硬约束**——任何 PR 都参照评估。

## 核心：低消耗 + 高性能（四维齐看）

性能不只是 wall time。串行改并发即使 real 砍半，user time 暴涨多核打满 / RSS 涨百兆，定位就崩了。**任何性能改动 SHALL 同时验证**：

1. **wall time（real）** — 用户感知耗时
2. **user/sys time** — CPU 实际工作量
3. **user/real ratio** — 进程平均占用核数（**非系统 CPU 百分比**；`0.5` ≈ 8 核机 6%、`1.0` = 单核满载 ≈ 12.5%、`4.0` = 4 核满载 ≈ 50% 风扇起转）
4. **max RSS** — 进程内存峰值

测量：`/usr/bin/time -lp <bench-cmd>`（macOS）/ `/usr/bin/time -v`（Linux）。系统 CPU 看 Activity Monitor 或 `top -pid $(pgrep claude-devtools-tauri)`。

## 性能预算（基线 v0.4.10 · 2026-05-16 · 30 project × 538 session）

**回归 > 阈值即拒**。

| 路径 | wall 预算 | CPU（user/real） | RSS 预算 | 当前基线 |
|---|---|---|---|---|
| 冷启动 list_repository_groups | < 200ms | ≤ 0.5（爆发 ≤ 1.5）| < 80 MB | bench 95ms · user/real=**0.13** / RSS 59MB |
| 冷启动 list_projects | < 150ms | 同上 | 同上 | 87-93ms |
| 大会话 get_session_detail（10k 消息）| < 800ms | ≤ 0.5 | < 200 MB | 1221 msg → 60-74ms · user/real=**0.17** / RSS 120MB |
| 首屏 sidebar 可见列表 | < 500ms | — | — | 待测 |
| Tauri IPC payload | < 1 MB（>1MB 须瘦身）| — | — | — |

**辅助工具系统 CPU 阈值**（人工 Activity Monitor 验，watcher / scanner / 后台 task PR 必测）：
- idle 稳态 < 2%（8 核 < 0.16 核）
- 后台扫描峰值 < 10% 短时间（< 1s）
- 用户交互峰值 < 15% < 200ms

**回归阈值**（任一即拒）：wall +20% / user +50%（real 没同步降）/ RSS +30% / bench user/real > 1.0 长时间（short burst < 50ms 放宽到 ≤ 1.5）/ user/real 跨过 0.5 且 real 降 < 30%

## bench 入口

- `cdt-api/tests/perf_cold_scan.rs` — 冷启动 scan + grouper
- `cdt-api/tests/perf_get_session_detail.rs` — 大会话首次打开
- `/perf-bench` skill — 自动跑 + 解析 + verdict
- `bash scripts/run-perf-bench.sh` — 四维 baseline gate runner（`--bench <name>` 单跑 / `--runs N` 调样本数 / 报告写到 `target/perf-report.json`）

新增关键路径 SHALL 加对应 bench + 把基线填上表。**每次会话开始 + 用户问"为什么慢" + 发版前** SHALL 先跑 bench 拿数据再讨论方向。

## CI 自动 gate

`.github/workflows/perf.yml` 在每个 PR + push to main 跑 `scripts/run-perf-bench.sh` 校验 baseline schema + binary 链路通畅。

**CI runner 无 `~/.claude/projects/` corpus**——两个 bench 在 CI 内部 `if !projects.exists() { return }` 跳过，本 workflow 仅作 **smoke 校验**。**真实四维 gate 由 dev 在本地跑**——PR push 前 SHALL 跑 `bash scripts/run-perf-bench.sh`。

噪声策略 + baseline 更新流程的详细说明见 `scripts/run-perf-bench.sh` 头部注释 + `tests/perf-baseline.json` 每个 bench 的 `$comment` 字段。核心：**min-of-N 抑噪**（默认 N=5），**禁止**为让 CI 过而调高 baseline——baseline = 算法真实成本。

## PR Perf impact 模板（强制）

涉及 `cdt-discover/` / `cdt-api/src/ipc/` / `cdt-analyze/` / `tauri.conf.json` / 引入子进程 spawn / hot loop file I/O / hot loop JSON parse 的 PR，SHALL 在描述里贴：

```markdown
## Perf impact
- 关键路径：[xxx]
- wall：基线 a ms → 本 PR b ms（±%）
- user：c.cc s → d.dd s（±%）
- sys：e.ee s → f.ff s（±%）
- max RSS：N MB → M MB（±%）
- user/real：0.xx → 0.yy
- 数据：/usr/bin/time -lp <cmd> 输出
```

四维缺一可拒。豁免：纯 docs / 注释 / typo / CI 配置。

## 反模式清单（**严禁**引入，违反即拒）

**wall time 类**：
- for-loop 串行 spawn 子进程 / 串行 file I/O — 用 `join_all` + `Semaphore` 并发；优先看能否换纯 fs 调用
- 每次 IPC 重扫文件 / 重算 chunk — 按 `FileSignature` 内存 cache（参照 `MetadataCache`）
- async fn 里调 `std::fs::*` / `Command::output().wait()` — 阻塞 tokio worker，用 tokio 异步版
- 算法 O(N²) 在 N > 100 时
- IPC payload > 1 MB 不瘦身 — 走 `OMIT_XXX const + xxxOmitted: bool + get_xxx_lazy IPC` 模式（详 `src-tauri/CLAUDE.md::IPC payload 瘦身模式`）

**CPU 类**（与 wall time 同等重要）：
- CPU-bound 路径串行改并发不限流 — `Semaphore` 限到 ≤ CPU 核数 / 4；判断 I/O-bound vs CPU-bound 看 baseline `user/real`（< 0.3 强 I/O / > 0.7 CPU-bound）
- 加完并发不测 user time — 必须四维齐看
- hot loop 隐式 `clone()` 大对象 — 优先 `Arc<>` / `&` / `mem::take`
- 同步循环里 `serde_json::from_str` / `to_string` 大 JSON

**内存类**：
- cache 仅设 count cap 不设 byte cap — 必须 `current_bytes: AtomicUsize` + `max_bytes` 双闸门
- 永久持有全量 `Vec` 的 Map — 加 LRU + TTL；流式状态机替代收集后判一次
- IPC 整页 base64 inline — 走 `asset://` URL 或 lazy IPC
- `broadcast::channel(N)` capacity 过大 — 默认 128 起步，新加 subscriber 时 grep 退订路径防泄漏

## codex 二审性能视角

性能相关 PR 的 codex prompt SHALL 显式列：
- for-loop spawn / 串行 await（应 join_all + 限流）
- hot path 缺 cache / 重复 IPC payload 字段
- `Semaphore` 限流是否合理
- hot path 隐式大对象 clone
- 新 cache 有 byte cap 否；broadcast capacity 是否过大；subscriber 退订路径
- 算法复杂度评估；PR 描述四维 perf 数据是否齐全

## 历史与教训

历史优化轨迹：`git log --grep="feat(perf)\|perf("`。**关键教训**：有文件可读时绝不 spawn 子进程（syscall 比 process spawn 快 1000×）；hot path SHALL cache by signature；并发不限流不如串行。

## Hook 性能（每个 Bash 工具调用串行跑）

预算：cold path（99% 调用 exit 0）单 hook < 60ms / 总和 < 250ms。验证：`just bench-hooks`。

硬约束：
- matcher 已 gate `tool_name` → hook 内不要重判
- 99% 路径用 `case "$input" in ...) ;; *) exit 0 ;; esac` 预判（bash 内置 0 fork）
- JSON 解析用 `jq`（25ms）不用 `python3`（60ms），失败 fallback `sed`
- 读 stdin 用 `$(</dev/stdin)` 不用 `$(cat)`（省 25ms cat 子进程）
- 重命令（git / openspec / cargo）在所有快速路径后最后一步才跑

物理下限 ~56ms（bash 3.2 启动 28 + stdin 读 25 + init 3）。再低需装 bash 5.x。

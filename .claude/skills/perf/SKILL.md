---
name: perf
description: 性能统一入口——含 SessionDetail 首屏 bench 跑分（references/bench-session-detail.md）和 idle CPU 系统性诊断（references/diagnose-idle-cpu.md）。**只要**用户提到性能 / 慢 / 卡顿 / 风扇起转 / IPC 慢 / SessionDetail 首屏 / 加载耗时 / payload 大 / 大会话 / idle CPU 高 / 后台烧 / 偶尔卡 / 线程多 / 进程占 X% CPU / 电池掉得快，或显式 `/perf` / `/perf-bench` / `/perf-cpu-diagnose`，**都用这个 skill**——不要自己手跑 cargo test / sample / top 后乱解读数字。
---

# perf

## 子模式选择

| 症状词 | 读哪个 reference |
|---|---|
| 首屏 / 加载耗时 / payload / IPC 慢 / 大会话 / SessionDetail | [references/bench-session-detail.md](references/bench-session-detail.md) |
| idle CPU 高 / 风扇 / 后台烧 / 偶尔卡 / 线程多 | [references/diagnose-idle-cpu.md](references/diagnose-idle-cpu.md) |
| 不确定 | 先问用户"是打开会话慢，还是应用挂着不动 CPU 也高？"按答案路由 |

bundled script：`scripts/sample-cpu.sh <PID> [duration]` — 自动跑 sample + top + 栈分类，给 idle 子模式用。

复杂诊断（多个不确定因子）走渐进多轮 codex 模式，模板见 `.claude/templates/codex-prompt-progressive-diagnosis.md`。

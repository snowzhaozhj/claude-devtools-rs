# codex 渐进多轮诊断 prompt 模板

## 何时用

复杂诊断 / design 多决策因子 / 跨多个不确定假设的判断。**不**用于单点 PR 二审（用 [codex-prompt-pr-review.md](codex-prompt-pr-review.md)）/ 单 design 决策（用 [codex-prompt-design-review.md](codex-prompt-design-review.md)）。

## 核心原则

1. **每轮 narrow focus 一个维度**，不要一次性 dump
2. 用 `SendMessage` 接续同 agentId 复用 context，不重读
3. 每轮 prompt 5 件套：完整事实 / 自己初判 / 3-5 个具体追问 / "如果方向错请直接说"兜底 / 回答要求（直接观点 + 中文 + 不堆背景）

## 4 阶段 narrow focus（按需调用，不必每次跑全）

| 阶段 | Focus | 触发 |
|---|---|---|
| 1 initial | 验证假设 + 挑盲点 | `Agent({...})` 起 codex |
| 2 数值校准 | 具体配置数值 + 实验设计 | 阶段 1 修正方向后 |
| 3 时间线归因 | 回归源 vs 历史代码 | 用户给新基线 / git log 锁时间窗 |
| 4 决策矩阵 | ROI 优先级 + 推荐路径 | 完整问题清单出来后 |

## 实战 case study

本仓 v0.5.6 → v0.5.8 idle CPU 16% 诊断，4 轮各自抓到的关键修正：

| 轮 | 我的初判 | codex 修正 |
|---|---|---|
| 1 | 287 线程 = CPU 高 | condvar wait 不烧 CPU；线程数 ≠ CPU |
| 2 | `thread_keep_alive=3s` | 缩短是负优化，应延长 60s |
| 3 | 双 runtime 是首因 | 双 runtime 是历史代码不是回归源 |
| 4 | 修后端就够 | WebView 13.4% 必须单列 P0 |

会话相关 commit：`git log --grep="codex" --grep="perf"`。

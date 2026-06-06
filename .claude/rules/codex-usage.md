# codex（异构推理）

同一推理引擎抓不到自己的盲点。调用：`Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`。prompt 模板：`.claude/templates/codex-prompt-*.md`。

## 核心原则

- **默认调用 + trivial 豁免**：PR push / design / explore 分叉默认调 codex，只有满足豁免条件才跳过。跳过时必须写 `Codex skipped: <reason>`
- **角色：评估者/攻击者**，不是生成者。挑战方案、构造反例、找逻辑盲点
- **最小上下文 prompt**：改动意图（1 句）+ 精简 diff + 关键不变量 + 输出格式约束
- **每个 finding 要求**：具体行号 + 复现路径 + 为什么现有测试没抓到
- **接续不重起**：多轮用 `SendMessage` 接续同一 subagent

## 触发点

| # | 时机 | 可观察信号 | 动作 |
|---|---|---|---|
| 1 | **PR push** | 默认调；高风险命中则禁止豁免 | 逻辑二审 |
| 2 | **rescue** | 同一问题 3 次尝试未解决 / 30min 失败数未减少 | 诊断 |
| 3 | **design 完成** | 默认调；高风险命中则禁止豁免 | 决策审 + 魔鬼代言人 |
| 4 | **对抗验证** | diff 涉及并发/状态机/缓存/错误恢复/配置组合/async 生命周期 | 构造非法状态序列 |
| 5 | **spec/scenario** | 任一 scenario 无对应 test 名映射 | 攻击式找漏 |
| 6 | **重构** | 文件 rename/move/split 影响 >1 个生产文件 | 语义假设断裂 |
| 7 | **perf 回归** | bench wall +20% / user/real 跃迁 / 改动命中 perf 路径但未跑 bench | 根因定位 |
| 8 | **error 变更** | 新增/删除 error variant / 改 `?` 传播 / 改 error→IPC 映射 | 边界完备性 |
| 9 | **explore 分叉** | 出现 ≥2 可行方案且选择影响下列任一：数据模型/模块边界/IPC/持久化/async/性能/用户可见行为 | 对 leading option 做对抗质询 |

## 高风险触发器（#1 #3 共用，命中禁止豁免）

IPC 字段/命令/payload 变化 / 跨 ≥2 cap 或 crate / perf 关键路径 / 状态机-并发-缓存-调度 / UI 重构（≥3 新组件或改导航/持久化状态）/ BREAKING / async lifecycle（spawn↔drop↔cancel）/ serde 持久化格式变更。

## trivial 豁免（全部满足才可跳过）

- 改动 <50 行且仅涉及注释/文案/格式/测试快照/单文件局部修复
- 不改 public API / IPC / serde / error 边界 / async / 性能路径 / 状态机 / 缓存 / UI 结构
- 不跨 crate/cap 边界，不移动/拆分生产文件
- 相关测试已跑通（无法跑测试 → 不得豁免）

## 魔鬼代言人（#3 追加）

design review prompt 末尾固定追加：这个设计最先会在哪断？扩展瓶颈在哪？

## explore 分叉（#9 补充）

不用"是否明显/复杂/值得"做判断。只要 explore 中讨论或隐含排除了 ≥2 方案，且选择影响上表列出的 7 项任一，就 SHALL 调 codex。prompt 须列出：leading option / rejected option(s) / 选择理由 / 最担心的假设。

## 对抗式验证（#4 详述）

- **并发**：Mutex/Semaphore/broadcast/CancellationToken → deadlock/race 序列
- **状态机**：enum 转移 → 非法态输入序列
- **缓存**：invalidation → stale 数据时序
- **错误恢复**：retry/fallback → "恢复再失败"嵌套
- **配置组合**：feature flag 交互 → 未测试组合
- **async 生命周期**：spawn 后 drop handle / 取消时资源泄漏 / channel close 后发送

## 二审找到 bug 后

全部修完合一个 commit 再 push → SendMessage 接续验证 → 二者都过才 push。

## 与 pr-review-toolkit 互补

codex = 异构推理（跨模型查逻辑盲点）；pr-review-toolkit = 同模型专项 agent。并行不替代。

用 `silent-failure-hunter`（静默失败）、`pr-test-analyzer`（测试质量）、`code-simplifier`（代码简化）、`type-design-analyzer`（类型设计，可选）。不用 `code-reviewer`（与 codex 重叠）、`comment-analyzer`（项目不写注释）。

触发条件详见 `opsx-apply-cadence.md::发布尾段 step 9`。

## 调用记录

codex 调完报 "codex: N 个 bug / 0 问题"。pr-review-toolkit 同理。

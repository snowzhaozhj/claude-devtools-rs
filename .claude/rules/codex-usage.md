# codex（异构推理）

同一推理引擎抓不到自己的盲点。调用：`Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`。prompt 模板：`.claude/templates/codex-prompt-*.md`。

## 核心原则

- **角色：评估者/攻击者**，不是生成者。让 codex 挑战方案、构造反例、找逻辑盲点
- **最小上下文 prompt**：改动意图（1 句）+ 精简 diff + 关键不变量 + 输出格式约束。不贴完整文件、不贴背景口号
- **每个 finding 要求**：具体行号 + 复现路径 + 为什么现有测试没抓到
- **接续不重起**：多轮用 `SendMessage` 接续同一 subagent，不起新 agent 重读 context

## 触发点

| # | 何时 | 条件 | 做什么 |
|---|---|---|---|
| 1 | **PR push** | 高风险命中（见下） | 逻辑二审 |
| 2 | **卡住** | 同一问题 30min+ | rescue 诊断 |
| 3 | **design 写完** | 高风险命中 | 决策审 + 魔鬼代言人 |
| 4 | **对抗式验证** | 并发/状态机/缓存/错误恢复/配置组合 | 构造非法状态序列 |
| 5 | **spec/scenario** | scenario↔test 不全 | 攻击式找漏 |
| 6 | **重构** | 文件 rename/move/拆模块 | 语义假设断裂 |
| 7 | **perf 回归** | bench wall +20% 或 user/real 跃迁 | 根因定位 |
| 8 | **error 变更** | 新 error variant / 改 `?` 传播链 | IPC 边界完备性 |

## 高风险触发器（#1 和 #3 共用）

任一命中即调：IPC 字段改 / 跨 ≥ 2 cap / 性能关键路径 / 状态机-并发-缓存 / UI 重构（≥ 3 新组件）/ BREAKING / async task 生命周期（spawn↔drop↔cancel）/ serde 持久化格式变更。

**豁免**：bump version / 纯 docs / 单点 typo / CI 配置微调（PR 描述留"未跑 codex（理由）"）。

## 魔鬼代言人（#3 追加）

design review prompt 末尾固定追加：
1. 这个设计最先会在哪断？
2. 扩展瓶颈在哪？

## 对抗式验证（#4 详述）

不限于并发。任何有隐含假设且编译器/测试难穷举的域：
- **并发**：Mutex/Semaphore/broadcast/CancellationToken → deadlock/race 序列
- **状态机**：enum 转移 → 非法态输入序列
- **缓存**：invalidation → stale 数据时序
- **错误恢复**：retry/fallback → "恢复再失败"嵌套
- **配置组合**：feature flag 交互 → 未测试组合
- **async 生命周期**：spawn 后 drop handle / 取消时资源泄漏

prompt：`"<code> + <机制>，构造 3 个具体序列让系统进入非预期状态，标注关键时间点，忽略类型系统已保证的不变量。"`

## 二审找到 bug 后

全部修完合一个 commit 再 push → SendMessage 接续验证 → 二者都过才 push。非阻塞建议随下次实质修复提交。

## 不用 codex

- ideation（用魔鬼代言人挑战方案即可）
- 跨 PR 语义冲突（ad-hoc）
- IPC 字段对齐（投资 codegen）

## 调用记录

每次调完说一句 "已让 codex 二审，找到 N 个 bug / 0 个问题"。

# codex 在研发流程中的角色

claude-devtools-rs 的二审与协同推理优先用 **codex（GPT-5.4 异构推理）**，不是再开一个 Claude subagent——后者本质还是同一推理引擎，捉不到自己的盲点。

调用方式：用 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`。**不要**新建 `/codex-*` skill 重新封装——一层薄包装反而让触发条件僵化。

下面是各阶段 codex 角色 + 触发判断。

## 1. PR commit 之后：二审

### 何时必须跑 codex 二审

- 改动涉及 **行为契约**（IPC 字段语义 / 后端算法 / 状态判定 / 数据 omit 策略 / Tauri command 协议）
- 改动涉及 **性能 / 节流 / 缓存 / 并发**（典型：debounce / LRU / in-flight 合并 / 后台任务取消）
- 改动跨 **5 个文件以上** 或 **200 行以上**

### 何时既有 `/code-review` 就够

- CLAUDE.md adherence 自检（命名 / 注释 / clippy pedantic）
- 纯视觉对齐 / 单点样式修复 / Trigger CRUD
- bump version / docs / chore

### 何时两个都跑

- 重大 PR（spec 改动 + 行为契约 + 跨多 capability）：**先 codex 二审找深逻辑 bug，再 `/code-review` 走 CLAUDE.md adherence + 落 PR comment**

### codex 二审的 prompt 模板（用 Agent 调用 codex:codex-rescue）

```
背景：[一句话讲改动目的 + 分支名]

诊断（如果有）：[改动想解决什么问题；省略可]

改动范围：
[列出文件 + 行数变化]

我已经验证的事：
- preflight 全绿（具体数字）
- 单测覆盖了 X / Y / Z

我希望你重点查的问题：
1. [具体怀疑点 1，最好带文件 + 行号]
2. [具体怀疑点 2]
...

约束：
- 只报你确认是 bug 或有数据支撑的设计漏洞，不要"建议"或"可以更优雅"
- 每个问题给：文件 + 行号 + 现状 + 为什么是 bug + 修法
- 中文，500 字以内
- 仓库根：/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs
- 当前分支：[branch]
```

**关键**：列出"具体怀疑点"——不要让 codex 漫无目的扫，那样找出来的多是泛泛建议。把你心里的不安交给它去验证。

### 二审找到 bug 后

- 全部修完再 push 第二个 commit（**不**事后处理，不留尾巴）
- 单测同步覆盖每个修复的 bug（codex review 里要求"补单测"段）
- commit message 标注 "修 codex review 找到的 N 个 bug"

## 2. 实现卡住：rescue

`codex:codex-rescue` 这个 subagent 描述里就写了"proactively use when stuck"。

判断标准：
- claude 同一个文件 / 同一个错误调试 30+ 分钟没进展
- 反复 grep 找不到符号 / 反复改测试还失败
- 对架构选择拿不准（A 方案 vs B 方案权衡不清）

**不要等用户喊**——感觉卡住主动调。Codex 会从全新视角 grep + 读代码，常常一刀切到根因。

## 3. design 阶段：决策风险二审

`/opsx:propose` 写完 design.md 之后，**行为契约 / 跨 capability / 性能** 类 change 调 codex 评：

- D1/D2/D3... 各决策的取舍 / 候选方案对比是否合理
- 风险点是否漏列
- spec delta 的 Scenario 是否可测试 / 是否漏边界

prompt 关键词："请审查 `openspec/changes/<slug>/design.md` 的决策合理性，特别是 D<n>，看候选方案对比是否漏掉了 [...]"。

不强制（design 阶段已经有 reviewer 角色——/opsx:explore 时的 thinking partner），但**重大决策**最好走一遍。

## 4. test 阶段：edge case 测试用例

claude 写完单测后，让 codex 看一遍 spec scenarios 给出对应 edge case：

- "spec scenario X 我用 [...] 测了，还有什么边界场景没覆盖？"
- "这个 LRU 的 evict 顺序，构造一个能在 Y 测出来的反例"

Codex 出 edge case 比 Claude 自检更狠（异构推理找盲点）。

不强制，但**含状态机 / 节流 / 并发 / 缓存淘汰**类改动 SHALL 至少跑一次。

## 5. archive 之前：spec delta 二审

`/opsx:archive` 之前，让 codex 审：

- spec delta 是否漏 SHALL/MUST 句（`openspec validate --strict` 只能查格式，查不出"语义漏覆盖"）
- 每个 Scenario 是否有对应测试（`spec-fidelity-reviewer` 已经覆盖一部分，但 codex 二审能找出"测试名对得上但行为没真覆盖"的伪覆盖）

判断：archive 前**自检三件事**全过即可跳过——
1. 全部 Scenario 都有 test 函数名能 grep 到（`spec-fidelity-reviewer` 自动）
2. 主 spec 的 SHALL 句没漏（人工过一遍）
3. tasks.md 全勾完

任意一项不全就跑 codex。

## 6. 不该用 codex 的场景

- **风格 / 命名 / 注释**：clippy + svelte-check + CLAUDE.md adherence 已经够，codex 二审会浪费 GPT-5.4 token
- **简单 bug 修复**：1 行改 1 行，跑测试就行
- **docs / readme 改动**：人工 review

## 调用频率与成本

codex:codex-rescue 调一次 ≈ 几十秒到几分钟 + GPT-5.4 token 消耗。**不要为了"流程仪式感"每个 PR 都调**——按上面分级触发。

记录：每次调 codex 在最终回复里说一句 "已让 codex 二审，找到 N 个 bug / 0 个问题"，留下审计痕迹。

## 与既有 /code-review 的关系

- `/code-review`（Anthropic 官方插件）：纯 Claude 多 agent 审 + gh PR comment 落地，强项是 CLAUDE.md adherence + 历史 PR 评论关联
- codex 二审：异构推理 + 深逻辑边界，强项是"我自己写的代码"的盲点

**互补不重复**：行为契约 / 性能 PR 走 codex 二审 + 之后 `/code-review` 落 PR comment；纯样式 / docs 只跑 `/code-review` 即可。

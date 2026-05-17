# codex 在研发流程中的角色

claude-devtools-rs 的二审与协同推理优先用 **codex（GPT-5.4 异构推理）**，不是再开一个 Claude subagent——后者本质还是同一推理引擎，捉不到自己的盲点。

调用方式：用 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`。**不要**新建 `/codex-*` skill 重新封装——一层薄包装反而让触发条件僵化。

下面是各阶段 codex 角色 + 触发判断。

## 1. PR commit 之后：二审

### 默认：所有 PR 都跑 codex 二审

push 第一个 commit 之后**默认调** `Agent({ subagent_type: "codex:codex-rescue", ... })`，无论改动大小 / 类型。理由：
- 纯样式 PR 也踩过坑（典型：本 PR `feat/tool-syntax-highlight` 的 `bat/cmd→powershell` 误映射、`Dockerfile.dev` 不走 special name——纯字典扩展也藏 bug）。
- codex 异构推理（GPT-5.4）的边际成本远低于"漏 bug 进 main 后回滚 / hotfix"的代价。
- 单 PR 调用一次 codex ≈ 几十秒到几分钟，不构成成本压力。

### 显式豁免（可跳过 codex CR）

只有以下场景**可以**跳过——但跳过时仍要在 PR 描述里写一句 "未跑 codex（理由：xxx）" 留痕：

- **bump version / 改 Cargo.lock / pnpm-lock.yaml**：纯版本号字面改动，无逻辑
- **docs / README / CLAUDE.md / 规则文件改动**：纯文本，无代码语义
- **单点 typo 修复 / 单点 i18n 文案改**：1-2 行字符串替换
- **CI / GitHub Actions 配置微调**：仅触发条件 / cache key 等，无新 step

### 触发判断流程

```
PR push 后
  ├─ 改动文件都属于"显式豁免"列表？ → 跳过 codex，PR 描述写明理由
  └─ 否则 → 跑 codex 二审（默认路径）
```

不再用"二选一"逻辑——codex 异构推理已覆盖 CLAUDE.md adherence + 深逻辑，`/code-review` 不再常规跑，只在想要"PR 评论历史可视化"的特殊场景手动调（例如某次 demo / 教学 PR 想留多 agent 审计痕迹）。

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
- **修完先跑第二轮 codex 验证才 push**（用同一 subagent，prompt 列出第一轮 bug + 我的修法 + 想让 codex 重点查的"修法是否真的解决"）。验证通过后再 commit + push；archive commit SHALL 是 codex 验证通过后才打的 PR 最后一个 commit。否则 PR 历史里挂着未验证的 fix，可能仍有 race / 边界 bug——本仓 PR #38 的 active_scans race 第一次修复就漏了 spawn/insert 之间的锁释放 window。

## 2. 实现卡住：rescue

`codex:codex-rescue` 这个 subagent 描述里就写了"proactively use when stuck"。

判断标准：
- claude 同一个文件 / 同一个错误调试 30+ 分钟没进展
- 反复 grep 找不到符号 / 反复改测试还失败
- 对架构选择拿不准（A 方案 vs B 方案权衡不清）

**不要等用户喊**——感觉卡住主动调。Codex 会从全新视角 grep + 读代码，常常一刀切到根因。

## 3. design 阶段：决策风险二审

`/opsx:propose` 写完 design.md 之后、**进 `/opsx:apply` 之前**调 codex 评。**默认强制**——理由：propose 阶段定下的 D1/D2/... 决策在 apply 阶段会扩散成几十处代码改动，事后发现 design 漏洞代价远高于 propose 阶段拦下；codex 异构推理擅长抓"前端拿不到 Phase 1 数据"这类后端/前端数据流断裂、"字段名前后写错"这类文档不一致，是 claude 自检的盲点。

**默认调（任一命中即调）**：
- 涉及 IPC 字段语义改 / 新增 / 删除
- 跨 ≥ 2 个 capability spec delta
- 性能关键路径改动（启动 / IPC 大 payload / 后端算法 O(N²) / 列表渲染）
- 状态机 / 节流 / 并发 / 缓存淘汰策略
- UI 重大重构（拆 ≥ 3 个新组件 / 改 ≥ 2 个核心组件）
- 含 BREAKING change 标注

**可跳过（同时满足）**：
- 单 capability + 单 Requirement 改动
- 纯文案 / 纯样式 / 单点 bug 修复
- 改动 ≤ 50 行预期 + 无新 IPC 字段
- design.md 只有 D1 一个决策且 reviewer 看一眼就能验证

**prompt 模板**（针对 design 阶段，与 PR push 后二审区分）：
```
背景：[一句话讲 change 解决什么问题 + slug]

诊断：propose 阶段已写完 design / spec delta / tasks，进 apply 前需要异构二审。

请审查的文件：[列文件路径]

我的具体怀疑点：
1. D<n> [具体决策] 有没有 [具体技术坑]
2. spec delta 有没有漏 SHALL/MUST 句、漏 scenario 边界
3. tasks.md 拆分有没有漏 IPC 字段 / 测试断言点 / fixture 同步
4. [其它领域知识相关怀疑]

约束：
- 只报你确认是 bug、设计漏洞、或文档不严的；不要"建议优化"
- 每个问题给：文件路径 + 行号（或章节）+ 现状 + 为什么是问题 + 修法
- 中文，800 字以内
```

codex 报问题后**先修 design / spec / tasks 三处文档**，再 validate strict 过，再进 `/opsx:apply`。修完不需要再跑一轮 codex 验证（与 apply 后的 PR push 二审不同）——文档修改 reviewer 一眼能看出对错，循环成本不值。

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

## 6. 调用频率与成本

每个 PR 默认调 codex 一次 ≈ 几十秒到几分钟 + GPT-5.4 token，相对漏 bug 进 main 的代价可忽略。

记录：每次调 codex 在最终回复里说一句 "已让 codex 二审，找到 N 个 bug / 0 个问题"，留下审计痕迹。

## 7. 与既有 /code-review 的关系

- `/code-review`（Anthropic 官方插件）：纯 Claude 多 agent 审 + gh PR comment 落地，强项是 PR 评论历史可视化
- codex 二审：异构推理 + 深逻辑边界，强项是"我自己写的代码"的盲点 + 跨语言/跨框架边界 case

**默认 codex，`/code-review` 仅按需手动调用**：codex 异构推理已覆盖 CLAUDE.md adherence + 深逻辑，常规 PR 跑 codex 就够；`/code-review` 仅在 (a) 想要 PR 评论历史 / 多 agent 审计痕迹，或 (b) codex 已审过但你对某细分维度仍不放心 想再叠一层 时手动调。同一份代码两个都跑不算重复——视角不同，但默认只跑 codex 即可。

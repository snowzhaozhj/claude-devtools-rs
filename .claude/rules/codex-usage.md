# codex 在研发流程中的角色

claude-devtools-rs 的二审与协同推理优先用 **codex（GPT-5.4 异构推理）**，不是再开一个 Claude subagent——同一推理引擎抓不到自己的盲点。调用方式：`Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`。**不要**新建 `/codex-*` skill 重新封装。

prompt 模板见 `.claude/templates/codex-prompt-pr-review.md`（PR 二审）/ `codex-prompt-design-review.md`（design 决策）/ `codex-prompt-progressive-diagnosis.md`（渐进多轮诊断）。

## 1. PR commit 之后：二审（默认调）

push 第一个 commit 并创建 PR 后**默认立刻调** codex 二审，且与 `/wait-ci` 后台 watch 并行跑，不等 CI 结束才开始。理由：codex 与 CI 互不依赖，串行会把 wall time 叠加；纯样式 PR 也踩过坑（`bat/cmd→powershell` 误映射、`Dockerfile.dev` 不走 special name 等纯字典扩展也藏 bug）；codex 边际成本远低于"漏 bug 进 main 后回滚 / hotfix"。

**显式豁免**（跳过时 PR 描述写"未跑 codex（理由：xxx）"留痕）：
- bump version / `Cargo.lock` / `pnpm-lock.yaml` 纯版本号改动
- docs / README / CLAUDE.md / 规则文件纯文本改
- 单点 typo / i18n 文案 1-2 行字符串替换
- CI / GitHub Actions 配置微调（仅触发条件 / cache key）

### 二审找到 bug 后

1. **全部修完再 push**（不留尾巴，单测同步覆盖每个修复）；若 CI 同时失败，把 CI 和 codex 问题合并成一个修复 commit，避免多轮空跑
2. **第二轮 codex 验证与本地验证并行**——用同一 subagent + `SendMessage` 接续，prompt 列出第一轮 bug + 修法 + "修法是否真解决"；同时跑本地 clippy/test/perf，二者都过再 push
3. push 修复后同时启动新一轮 CI watch 与（如有需要）codex 复核；archive commit SHALL 是 codex 验证通过后才打的 PR 最后一个 commit
4. 非阻塞建议（注释、文案、微小整洁）默认不单独 push；除非会影响 reviewer 理解，否则留到下一次实质修复或 archive 前一起处理

历史案例：PR #38 active_scans race 第一次修复漏了 spawn/insert 之间的锁释放 window，靠多轮 codex 才抓到。

## 2. 实现卡住：rescue（主动调）

`codex:codex-rescue` subagent 描述就写了"proactively use when stuck"。**不要等用户喊**——感觉卡住主动调。

触发：
- 同一文件 / 同一错误调试 30+ 分钟没进展
- 反复 grep 找不到符号 / 反复改测试还失败
- 对架构选择拿不准（A vs B 权衡不清）

## 3. design 阶段：决策风险二审（任一命中即调）

`/opsx:propose` 写完 design.md 后、进 `/opsx:apply` **之前**默认强制调。理由：propose 阶段定下的 D1/D2 决策在 apply 阶段会扩散成几十处代码改动，事后发现 design 漏洞代价远高于 propose 阶段拦下。

**默认调（任一命中）**：
- IPC 字段语义改 / 新增 / 删除
- 跨 ≥ 2 个 capability spec delta
- 性能关键路径（启动 / IPC 大 payload / O(N²) / 列表渲染）
- 状态机 / 节流 / 并发 / 缓存淘汰策略
- UI 重大重构（拆 ≥ 3 个新组件 / 改 ≥ 2 个核心组件）
- 含 BREAKING change 标注

**可跳过（同时满足）**：单 capability + 单 Requirement / 纯文案纯样式单点 bug / ≤ 50 行预期 + 无新 IPC 字段 / design.md 仅 D1 一个决策。

## 4. test 阶段：edge case（按需调）

claude 写完单测后让 codex 看 spec scenarios 给 edge case：`"spec scenario X 我用 [...] 测了，还有什么边界场景没覆盖？"`

不强制，但**含状态机 / 节流 / 并发 / 缓存淘汰**类改动 SHALL 至少跑一次。

## 5. archive 之前：spec delta 二审（条件跳过）

`/opsx:archive` 之前 codex 审：
- spec delta 是否漏 SHALL/MUST 句
- 每个 Scenario 是否有对应测试（`spec-fidelity-reviewer` 能查命名，codex 能查"测试名对得上但行为没真覆盖"的伪覆盖）

**自检三件事全过即可跳过**：
1. 全部 Scenario 都有 test 函数名能 grep 到
2. 主 spec 的 SHALL 句没漏（人工过一遍）
3. tasks.md 全勾完

任意一项不全就跑 codex。

## 6. 与 `/code-review` 的关系

- `/code-review`（Anthropic 官方）：纯 Claude 多 agent 审 + gh PR comment，强项是 PR 评论历史可视化
- codex：异构推理 + 深逻辑边界，强项是"自己写的代码"盲点 + 跨语言/跨框架边界

**默认 codex，`/code-review` 仅按需手动调**——常规 PR 跑 codex 就够。

## 7. 调用记录

每次调 codex 在最终回复说一句 "已让 codex 二审，找到 N 个 bug / 0 个问题"，留下审计痕迹。

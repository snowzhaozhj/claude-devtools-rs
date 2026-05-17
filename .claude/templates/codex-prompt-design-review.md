# codex design 阶段二审 prompt 模板

`/opsx:propose` 写完 design.md 之后、**进 `/opsx:apply` 之前**调：

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

codex 报问题后**先修 design / spec / tasks 三处文档**，再 `openspec validate <slug> --strict`，再进 `/opsx:apply`。**不需要**再跑一轮 codex 验证（与 apply 后的 PR push 二审不同）——文档修改 reviewer 一眼能看出对错，循环成本不值。

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

视觉契约交叉检查（design.md 涉及 UI 组件 / 视觉决策 / 用户感知行为时强制；纯后端 / IPC / 算法 / 数据流 design 跳过）：
- 对照 `DESIGN.md` 的 Named Rules（`The XXX Rule.` 形式），检查 design.md `## Visual Contract` 段引用的 Named Rule 是否真存在 + 决策是否与 Rule 一致；如有违反 SHALL 显式作为 `D-V<n>` 决策记录并选定"改 DESIGN.md 还是这次例外"
- 对照 `PRODUCT.md::Design Principles` / `Anti-references` / `Accessibility`，检查 design.md 决策是否违反产品级原则（特别是动效预算 / loading 中间态 / 装饰性元素 / a11y 焦点态）
- 历史教训：PR #177 / #270 propose 阶段 codex 二审通过但漏检 `DESIGN.md:198` + `PRODUCT.md::Design Principle 5`，直到 issue #256 性能诊断才发现 shimmer 是视觉契约违规——design 阶段拦下问题的回炉成本是 apply 阶段的 10×
```

codex 报问题后**先修 design / spec / tasks 三处文档**，再 `openspec validate <slug> --strict`，再进 `/opsx:apply`。**不需要**再跑一轮 codex 验证（与 apply 后的 PR push 二审不同）——文档修改 reviewer 一眼能看出对错，循环成本不值。

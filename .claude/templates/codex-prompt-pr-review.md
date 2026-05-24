# codex PR push 后二审 prompt 模板

`Agent({ subagent_type: "codex:codex-rescue", prompt: ... })` 调用：

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

视觉契约交叉检查（改动涉及 UI / `.svelte` / CSS / 视觉行为时强制；纯后端 / IPC / 算法 PR 跳过）：
- 对照 `DESIGN.md` 的 Named Rules（`The XXX Rule.` 形式，如 `The One Live Signal Rule` / `The Static-vs-Live Shape Rule` / `The Persistent Selection Is Quiet Rule` 等），检查改动是否违反任一硬约束
- 对照 `PRODUCT.md` 的 `Design Principles` / `Anti-references` / `Accessibility`（特别是动效预算 / loading 中间态 / 装饰性元素 / a11y 焦点态），检查改动是否违反产品级原则
- 历史教训：PR #177 / #270 引入 sidebar shimmer 时漏读 `DESIGN.md:198`「Skeleton placeholder 必须静态 opacity 占位，禁用 shimmer」+ `PRODUCT.md::Design Principle 5`「实时但不闪烁」，codex 二审通过但视觉契约违规——直到 issue #256 性能诊断才发现
```

**关键**：列出"具体怀疑点"——不要让 codex 漫无目的扫，那样找出来的多是泛泛建议。把你心里的不安交给它去验证。

**多轮验证**：codex 找到 bug 修完后，**先跑第二轮 codex 验证才 push**（用同一 subagent + `SendMessage`，prompt 列出第一轮 bug + 我的修法 + "修法是否真的解决"）。archive commit SHALL 是 codex 验证通过后才打的 PR 最后一个 commit。

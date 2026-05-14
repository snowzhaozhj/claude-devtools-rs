# Product

## Register

product

## Users

Claude Code 的重度使用者、插件维护者和团队内负责排查 AI 编程过程的人。用户通常在本地桌面环境中工作，需要快速回看多项目、多 worktree 下的 Claude Code 会话，确认工具调用、上下文注入、subagent 执行、通知触发和性能异常。

## Product Purpose

claude-devtools-rs 是一个可视化 Claude Code 会话执行的桌面工具。它扫描本机 `~/.claude/projects/`，把会话历史、实时刷新、工具调用、上下文面板、通知和全局搜索统一到一个可审计的开发工具界面中。成功的界面应让用户更快理解“Claude 刚才做了什么、为什么这么做、哪里需要介入”，而不是制造新的视觉负担。

## Brand Personality

克制、可信、工程化。界面语气接近 IDE / 调试器 / Linear 式工作台：信息密度高但不嘈杂，交互熟悉但不粗糙，状态反馈明确但不过度装饰。

## Anti-references

不要做成营销页、聊天玩具或霓虹终端。避免大面积高饱和品牌色、装饰性渐变、玻璃拟态、夸张动效、重复卡片墙和为了“好看”重造标准控件。工具调用、diff、日志、通知等信息必须优先保持可读、可扫、可定位。

## Design Principles

1. **审计优先。** 每个视觉决定都服务于快速定位会话、工具、状态和异常。
2. **熟悉即效率。** 采用桌面工具常见的 sidebar、tab、pane、command palette、inline disclosure，不为风格重造 affordance。
3. **密度有层次。** 支持高信息密度，但用边框、色阶、字号和折叠控制认知负荷。
4. **状态比装饰重要。** 颜色优先表达 selection、focus、success、error、warning、loading、ongoing，不做无意义点缀。
5. **实时但不闪烁。** 会话刷新、metadata patch 和通知更新应保持原地更新，避免 loading 中间态打断阅读。

## Accessibility & Inclusion

目标是桌面优先的可键盘操作产品 UI。交互控件应具备明确 focus-visible 状态、ARIA 语义和可达标签；颜色不能作为唯一状态通道。动效控制在 150–250ms，用于状态变化和空间关系，不做装饰性编排。当前代码中仍有少量 `svelte-ignore a11y_*` 的可点击非按钮区域，新增或重构时应优先收敛到语义按钮或补齐键盘行为。

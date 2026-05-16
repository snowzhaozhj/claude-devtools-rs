## Why

SessionDetail 在大型会话中即使没有新数据更新，持续上下滚动也会因为长 DOM 的 layout / paint 与无语言代码块的同步自动高亮检测造成明显 CPU 占用，用户观察到可超过 10%。本 change 通过低风险的 CSS containment 与高亮策略收敛，降低离屏内容对滚动路径的影响，同时避免引入完整 virtual list 的行为风险。

## What Changes

- 为 SessionDetail 对话流中的 chunk / message 级容器建立低风险渲染隔离策略：离屏内容 SHALL 使用浏览器原生 `content-visibility`、`contain-intrinsic-size` 与 `contain` 减少 layout / paint 成本。
- 收敛无语言 fenced code block 的高亮策略：大块或未声明语言的代码 SHALL 避免 `highlightAuto` 的同步语言检测，优先按 plaintext 渲染。
- 保持现有 lazy markdown、Mermaid、搜索、工具展开与滚动语义不变；不引入完整 virtual list，不改后端数据结构或 IPC。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- `session-display`: 增加 SessionDetail 滚动路径的容器级渲染隔离与无语言代码块高亮限制要求。

## Impact

- 影响前端：`ui/src/components/SessionDetail.svelte` 及相关样式、`ui/src/lib/render.ts`。
- 影响测试：补充渲染策略与高亮策略的 unit / browser 验证，确保搜索、lazy markdown、Mermaid 触发时机不回退。
- 不影响 Rust 后端、Tauri IPC 字段、OpenSpec 其他 capability 或数据模型。

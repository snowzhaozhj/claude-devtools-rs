## 1. 项目自动刷新（cdt-watch / cdt-discover / cdt-api / Tauri）

- [x] 1.1 定位现有 `FileWatcher` 对一级 project 目录创建与首个 `.jsonl` 文件创建的事件覆盖，并补对应 Rust 测试
- [x] 1.2 扩展 watcher / Tauri bridge 的事件 payload，使前端能识别项目列表需要重扫
- [x] 1.3 确保项目列表刷新调用复用现有 `project-discovery` 扫描规则，新增项目的 id/displayName/sessionCount 与冷启动一致
- [x] 1.4 更新 Sidebar / Dashboard / 项目选择入口，在项目刷新事件后静默重拉项目列表并保持现有选择状态

## 2. 工具执行数据与错误展示（cdt-core / cdt-analyze / ui）

- [x] 2.1 验证 `ToolExecution` IPC 是否稳定暴露 `startTs`、`endTs`、`isError`、`output`，缺失时补字段与 IPC contract 测试
- [x] 2.2 实现前端工具耗时格式化，主会话工具项与 subagent ExecutionTrace 工具项共用同一展示规则
- [x] 2.3 实现失败原因提取与展示，覆盖文本 output、结构化 `error/message/stderr` 字段和 JSON fallback
- [x] 2.4 补充工具失败原因与 pending/完成耗时的单元测试或组件测试

## 3. Edit diff 预览与语法高亮（ui）

- [x] 3.1 从 Edit 工具 input 的 `file_path` 推断 highlight.js 语言，未知语言降级为纯文本
- [x] 3.2 调整 `DiffViewer` / `EditToolViewer`，在保留 added/removed/context 样式与双列行号的同时渲染高亮内容
- [x] 3.3 覆盖 old/new、纯新增、未知扩展名三类 diff 预览测试

## 4. 工具结果展开性能（ui）

- [x] 4.1 定位展开工具结果的同步重渲染热点，确认 markdown/highlight/JSON stringify 触发点
- [x] 4.2 将工具展开体改为按需渲染并缓存派生结果，折叠状态不执行重内容渲染
- [x] 4.3 对大型文本输出复用 lazy markdown 或等价视口触发机制，避免首次展开阻塞主线程
- [x] 4.4 补充大输出展开的回归测试或手动验证步骤

## 5. 验证与收尾

- [x] 5.1 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 5.2 跑 `cargo fmt --all`
- [x] 5.3 跑相关 Rust crate 测试与 `npm run check --prefix ui`
- [x] 5.4 跑 `openspec validate runtime-tool-ui-updates --strict`
- [x] 5.5 启动 UI/桌面或 mock fixture 手动验证新增项目刷新、Edit diff、高亮、subagent 工具耗时、失败原因与大输出展开体验

## Why

原版 claude-devtools 已支持查看 Claude memory，用户可以在 DevTools 内快速核对 `MEMORY.md` 索引和各条 memory 文件内容。Rust 端口当前缺少这个入口，用户需要离开应用去文件系统查看，无法对齐原版调试与审阅体验。

## What Changes

- 新增 Memory 查看能力：侧边栏显示 `Memory` 入口和 memory 条目数量，点击后打开单例 Memory tab。
- 后端新增读取用户 memory 目录的 IPC：返回 memory layers 列表、默认选中文件、单个 memory 文件内容。
- 前端新增 Memory tab 类型、API 类型、store 入口与 `MemoryView` 页面。
- Memory 页面左侧展示 layers 列表（包含 index `MEMORY.md` 和每个 memory 文件摘要），右侧渲染选中文件 Markdown 内容。
- Memory 页面提供文件下拉切换与复制当前内容操作。
- 不改变 memory 文件写入语义；本 change 只读展示。

## Capabilities

### New Capabilities
- `memory-viewer`: 覆盖应用内发现、列出、读取并展示 Claude memory 文件的行为契约。

### Modified Capabilities
- `tab-management`: 新增 Memory 单例 tab 类型，要求打开 Memory 时复用已有 Memory tab。
- `sidebar-navigation`: 新增 Sidebar Memory 入口及数量展示。
- `ipc-data-api`: 新增 Memory 只读 IPC 方法及序列化字段契约。

## Impact

- 后端：`crates/cdt-api` 增加 memory 数据类型与 `LocalDataApi` 只读方法；`src-tauri` 增加对应 command 和 invoke handler。
- 前端：`ui/src/lib/api.ts`、`tabStore.svelte.ts`、`Sidebar.svelte`、`TabBar.svelte`、新增 Memory view/组件和 mock fixture。
- 测试：新增 Rust IPC contract 覆盖 camelCase 字段和 command 清单；新增 UI 单测/必要 e2e 覆盖 Memory tab 打开、文件切换、Markdown 渲染与 copy 入口。

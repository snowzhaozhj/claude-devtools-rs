## Why

当前 Settings → General → 数据目录切换把“当前目录”和“最近使用”混在下拉控件里：当当前 `claudeRootPath` 不在 `recentRoots` 或为默认目录时，最近使用下拉会显示为空白；切换成功后前端只刷新项目列表，已打开的 session / memory tab 与若干 root-scoped 缓存仍可能保留旧 root 内容，让用户感觉“点了但没刷新”。

这次改动把数据目录切换收敛为轻量、明确的 source switcher，并在前端建立一次 root switch 边界：保存成功后关闭旧 root 上下文、清 root-scoped 缓存、只刷新一次项目数据。

## What Changes

- Settings 数据目录区改为轻量展示：只显示当前目录一次，右侧用小字标记“默认”或“自定义”。
- 最近使用改为轻量列表而非下拉：只列其它可切换目录，过滤当前目录；过滤后为空则隐藏整段。
- 手动输入改为按钮行原地替换输入行：点击“输入路径”不插入新块，避免布局跳变；Enter 应用、Esc 取消。
- 切换 root 不弹确认 modal；仅在存在已打开 root-scoped tab 时显示提示：“切换会关闭当前会话 tab，并回到工作台。”
- root 切换成功后由 App 层统一协调：关闭 session / memory tabs，回 Dashboard，清 root-scoped 前端缓存，并一次刷新项目数据。
- 不新增后端 IPC / Tauri command；沿用现有 `update_config("general", { claudeRootPath })`，后端已有 runtime reconfigure scanner / watcher 能力。
- 性能约束：不扫描 `recentRoots`，不展示历史 root 的项目数 / 会话数，不让多个组件各自重扫。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `settings-ui`: 数据目录控件从下拉式 MRU 改为轻量当前目录 + 最近列表 + 原地输入路径，并定义切换反馈与错误态。
- `tab-management`: 数据 root 切换成功后 SHALL 关闭 root-scoped tabs 并回到工作台，避免旧 root session / memory 内容继续展示。
- `sidebar-navigation`: 数据 root 切换成功后 SHALL 清理 root-scoped session list / memory 缓存并重新加载当前 root 的 project/group 数据，避免旧 root 缓存 hydrate。

## Impact

- 前端：`ui/src/routes/SettingsView.svelte`、`ui/src/App.svelte`、`ui/src/lib/tabStore.svelte.ts`、`ui/src/lib/projectDataStore.svelte.ts`、`ui/src/lib/sessionListStore.svelte.ts`、`ui/src/components/Sidebar.svelte`。
- 测试：更新现有 `SettingsView.dataRoot.test.svelte.ts`，新增 root switch 状态清理相关单测；必要时补 Playwright / mock fixture 覆盖“切换后回工作台”。
- OpenSpec：修改 `settings-ui`、`tab-management`、`sidebar-navigation` 行为契约。
- 后端：不新增 IPC / HTTP route；不改变 `configuration-management` 的 `claudeRootPath` / `recentRoots` 语义。

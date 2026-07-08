## Context

`flexible-data-root` 已把数据根目录切换能力接入配置层：`general.claudeRootPath = null` 表示默认 `~/.claude`，非空值表示自定义 root；`general.recentRoots` 记录历史根目录供 UI 快速切换。后端 `update_config("general", { claudeRootPath })` 成功后会 runtime reconfigure scanner / watcher，Tauri 内嵌 HTTP server 复用同一个后端实例，不需要重启 server 或浏览器全页 reload。

当前问题集中在前端交互与状态边界：Settings 数据目录块把“当前值”和“最近使用”放进同一个下拉，当前值不在候选项时会出现空白控件；切 root 成功后只触发项目列表刷新，已打开 session / memory tab、tab session cache、session list SWR cache、Sidebar memory cache、selected group 等仍可能残留旧 root 上下文，造成“点了但没刷新”的用户感知。

用户已确认设计方向：轻量、少废话、避免误操作但不过度确认；不做重面板，不弹确认 modal，不展示历史 root 的项目数 / 会话数，不扫描历史 root。

## Goals / Non-Goals

**Goals:**

- 数据目录 UI 更轻：当前目录只显示一次，最近列表只显示可切换的其它 root。
- 手动输入不常驻、不弹窗：点击“输入路径”后按钮行原地替换为输入行，避免布局跳变。
- 切 root 成功后建立前端 root switch 边界：关闭旧 root 的 session / memory tabs，回 Dashboard，清 root-scoped caches，一次刷新项目数据。
- 保持性能可控：不扫描 `recentRoots`，不让多个组件各自重扫。
- 沿用现有后端配置更新和 runtime reconfigure，不新增 IPC / Tauri command。

**Non-Goals:**

- 不把数据源提升为全局一等切换器 / 多源管理器。
- 不聚合多个 root，不并行展示多个 root 的项目。
- 不展示历史 root 的项目数、会话数、健康状态或最近活动时间。
- 不改变 `configuration-management` 的 `claudeRootPath` / `recentRoots` 持久化语义。
- 不让 background jobs 随数据 root 切换；jobs 仍是 claude-devtools 自身的默认 `~/.claude/jobs` 队列，本 UI 文案只提 projects / todos。

## Decisions

### D1：用轻量当前行 + 最近列表替代 MRU 下拉

数据目录块显示为：当前路径主文本 + 右侧小字“默认”/“自定义”；按钮行 `[选择…] [输入路径] [恢复默认]`；最近列表只显示过滤掉当前 root 后的其它候选，行内只有路径 + `切换`。

- **候选 A（选中）轻量列表**：路径可以完整成为主信息，当前值不在候选项时也不会出现空白控件；最近列表不重复当前项，减少噪音。
- **候选 B 修下拉 fallback**：能修空白按钮，但下拉仍把“当前值”和“历史候选”混在一起，路径截断也难读。
- **候选 C source picker / popover**：防误操作更强，但多一步，当前需求只有默认 + 少量 MRU，偏重。

### D2：手动输入用按钮行原地替换，错误态才允许下推

默认按钮行保持紧凑；点击“输入路径”后同一行替换为输入框 + `应用` + `取消`，输入框预填当前值并聚焦全选。Enter 应用，Esc 取消。保存失败时不收起，并在输入框附近显示错误。

- **候选 A（选中）原地替换**：展开动作不插入新块，不导致“最近”列表跳动；错误态下推是用户提交后的反馈，可以接受。
- **候选 B 常驻输入框**：占空间且把高级路径输入变成主路径，视觉偏重。
- **候选 C modal / popover 输入**：上下文分离，且“输入路径”不是危险操作，不值得弹层。

### D3：切换成功后由 App 层统一协调 root switch

SettingsView 负责发起 `update_config` 并展示本地编辑状态；保存成功后通过语义事件或 callback 通知 App。App 持有全局 chrome / pane / selected group 语境，负责执行 root switch：重置 workspace tabs、清 root-scoped caches、清 selected group、触发一次 project refresh、选择新 root 的首个 group 或保持 Dashboard 空态。

- **候选 A（选中）App coordinator**：状态边界集中，避免 SettingsView 直接 import 多个 store 到处清理。
- **候选 B SettingsView 自己清**：短期少文件，但 SettingsView 会知道 tab/pane/sidebar/project store 细节，耦合过高。
- **候选 C 全页 reload**：最简单但体验粗糙，且后端已支持 runtime reconfigure，不需要用 reload 掩盖前端状态问题。

### D4：新增前端内部 API，不新增后端 IPC

需要新增的是前端内部状态 API：

- `tabStore`：提供 reset workspace 的能力，清 pane tabs、tab session cache、tab UI state，让主区回 Dashboard。
- `sessionListStore`：提供清 session list SWR cache 的能力。
- `projectDataStore`：提供 root switch reload / clear 语义，避免旧 in-flight 请求把旧 root 数据写回新 root 界面。

不新增 Tauri command / HTTP route。后端已在 `update_config` 成功后 runtime reconfigure scanner / watcher；前端问题不是后端不知道切 root，而是旧 UI state 没有边界。

### D5：root switch 成功前不清旧上下文

`update_config` 失败时，用户仍应留在旧 root 的有效界面；只有配置保存成功后才关闭旧 tabs、清缓存、回 Dashboard。

- **候选 A（选中）成功后清**：失败路径安全，用户不会因为 validation error 或 version mismatch 被扔到空工作台。
- **候选 B 点击即清**：视觉上立即反馈强，但保存失败会造成无意义破坏。

### D6：历史 root 不做预扫描统计

最近列表仅展示已知路径与切换动作，不展示项目数 / 会话数 / 最近活动。切换成功后只刷新当前 root 的 project data。

- **候选 A（选中）不扫描**：零额外 IO，不因 N 个历史 root 触发 N 次目录扫描。
- **候选 B 扫描历史 root 做富信息列表**：看起来更完整，但引入性能成本与状态复杂度，且路径可能不存在 / 位于慢盘 / 远端挂载。

### D7：Visual Contract

#### Surface Decision

保持在 Settings → General → 数据目录原位置，不新增全局入口、不新增 tab、不新增 modal。该控件仍是设置项，而不是多源管理器。

#### Visual Layer

- 遵循 `DESIGN.md::The Border Before Shadow Rule`：不引入卡片墙或阴影层级；用轻分隔、行高和 muted text 建立结构。
- 遵循 `DESIGN.md::The Status Owns the Color Rule`：默认 / 自定义使用低权重文本，pending / error 才使用状态色。
- 遵循 `DESIGN.md::The Tool Density Rule`：路径是主信息，按钮短文案，避免解释型长句常驻。

#### State Coverage

覆盖默认 root / 自定义 root / 最近为空 / 最近非空 / 输入展开 / 输入错误 / 保存中 / 切换成功 / 切换失败 / root-scoped tabs 存在提示。

#### DESIGN.md delta plan

本 change 不引入新的通用视觉 token；若 apply 后形成可复用的“设置项命令列表”模式，再在 archive 前评估是否提炼到 `DESIGN.md`。

## Risks / Trade-offs

- **[旧 in-flight project 请求写回旧 root 数据]** → projectDataStore 的 root switch reload 需要 generation / token 或 clear-before-fetch 语义，旧请求完成时不得覆盖 root switch 后的数据。
- **[SessionDetail destroy 回写旧 tab UI state]** → reset workspace 的实现顺序要避免“清 cache 后 onDestroy 又写回旧 state”；可先更新 pane layout 触发组件销毁，再清 tab store caches，或让 reset API 内部处理 destroy 回写窗口。
- **[清 sessionListStore 牺牲 SWR 体验]** → root switch 是低频显式操作，清缓存导致新 root 首次加载出现 skeleton 可接受；比旧 root 列表 hydrate 更正确。
- **[App 与 Sidebar 双重 refresh]** → root switch 由 App 统一刷新，Sidebar 不再作为唯一 root-change listener；保持 projectDataStore in-flight dedupe，但 root switch 路径须避免旧 inflight 的 first result 被当作新 root 成功结果消费。
- **[文案暗示 jobs 也切换]** → 数据目录说明只写 projects / todos，不写“所有数据”。

## Migration Plan

- 配置文件无需迁移。
- 先落内部 store reset/clear API 与 App root switch coordinator，再改 SettingsView UI 调用该路径。
- 更新 Settings 数据目录单测，补 root switch 后 workspace reset / cache clear 的单测。
- 回滚时可恢复旧 SettingsView 下拉 UI；新增 store API 若未被使用可保留为无副作用内部能力或删除。

## Open Questions

无。当前用户已确认轻量 UI 方向与“切换后关闭当前会话 tab 并回工作台”的交互取舍。

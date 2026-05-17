## Context

`SessionDetail.svelte::ftime()` 当前内联 `toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: true })`，显示 "上午 X 点 XX 分 XX 秒"。无开关、无可配置入口。同仓 `DisplayConfig` 已有 `showTimestamps` / `compactMode` / `syntaxHighlighting` / `fontSans` / `fontMono` 五个字段，时间格式属于自然漏配。

`Sidebar.svelte` / `CommandPalette.svelte` / `NotificationsView.svelte` 三处也有 `formatTime`，但全部走"相对时间"（刚刚 / Nm / Nh / Nd），>7d 才显示 `toLocaleDateString` 仅日期不含时分——不受 hour12 影响，本 change 不动这三处。

类似的 enum 配置先例：`config.general.theme`（`dark` / `light` / `system`）、`config.general.sessionClickBehavior`（`replace` / `new-tab`），二者已走 `update_<section>` 白名单 + ipc_contract round-trip 模式，本 change 复用同一模式。

## Goals / Non-Goals

**Goals:**
- `DisplayConfig` 新增 `timeFormat` 字段，可由用户通过 SettingsView 切换
- 默认值 `"24h"`，与中国大陆桌面 OS / IDE / 终端惯例对齐
- 旧配置文件（缺 `timeFormat` 字段）反序列化 → 落 `"24h"` 默认值
- `SessionDetail` 时间戳渲染统一走新的 `formatClock` 入口，便于未来扩展（如加秒级精度开关）

**Non-Goals:**
- **不**重写 `Sidebar` / `CommandPalette` / `NotificationsView` 的相对时间格式（"刚刚 / Nm / Nh"），它们本就不含 hour12 语义
- **不**支持日期格式配置（`YYYY-MM-DD` vs `MM/DD/YYYY` 等），scope 仅限"hh:mm:ss vs hh:mm:ss AM/PM"
- **不**做迁移脚本帮历史用户保留 12h 习惯——默认值变更是设计取舍（见 D2），用户重启后看到新默认即可手动改回
- **不**做 i18n 切换 locale（`zh-CN` 写死保留）

## Decisions

### D1: enum 序列化形式选 `"24h"` / `"12h"`

**候选**：
- A: `"24h"` / `"12h"` — 用户友好，IPC payload 自解释
- B: `"H24"` / `"H12"` — rust 枚举字符串风格
- C: bool `use12HourClock: true/false` — 极简但缺扩展性

**选 A**。理由：
- 与 IPC camelCase 风格一致（`getConfig().display.timeFormat === "24h"`）
- 与 SettingsView select option value 一一对应，前端无需再做映射
- 未来若加 `"24h-no-seconds"` 等变体可平滑扩展，比 bool 更可生长

### D2: 默认值从 `12h`（当前硬编码 `hour12: true`）改为 `24h`

**候选**：
- A: 默认 `"24h"` — 与桌面 OS 惯例对齐
- B: 保持默认 `"12h"` — 不破坏历史用户视觉习惯
- C: 默认 `"system"`（跟随系统 locale）— 看起来最稳但 `toLocaleTimeString` 不接受这种语义，需要查 `Intl.DateTimeFormat().resolvedOptions().hour12`，多一层 polyfill 风险

**选 A**。理由：
- 中国大陆 macOS / Windows / Linux 系统时间默认普遍 24h；IDE（VSCode / JetBrains）/ 终端 / Slack / 飞书等也均默认 24h；保持 12h 与目标用户群体的桌面上下文割裂
- 24h 节省"上午/下午"两字水平空间，`SessionDetail` 时间戳本就紧凑（与消息体并排）
- 用户调研缺数据时按"对齐外部惯例"决策，比"维持历史"信号更强
- **代价**：历史用户重启后 `SessionDetail` 时间戳从 "上午 X 点" 变 `14:23:05`。在 proposal 显式标 **BREAKING（user-visible 默认值变更）** 让用户在 release notes 看到，需要 12h 习惯的可去 SettingsView 切回

### D3: `formatClock` 入口只管时间不管日期

**候选**：
- A: `formatClock(date, hour12)` 仅返回 `hh:mm:ss` / `hh:mm:ss AM/PM`
- B: `formatDateTime(date, options)` 一把入口，同时管日期 + 时间
- C: 不抽公共入口，`SessionDetail` 内联 if-else

**选 A**。理由：
- 当前 scope 仅 `SessionDetail.ftime` 一个调用点切到配置驱动，B 方案过度设计
- Sidebar 等相对时间组件用 `formatTime`（自有逻辑），formatters.ts 的"日期相关"职责由现有 `formatDuration` / 新加 `formatClock` 各管一段，边界清晰
- C 方案让 SessionDetail 持有 hour12 派生逻辑，未来如再加调用点需要重复 → 拒
- 命名 `formatClock` 而非 `formatTime` 是为了避开与 Sidebar 等组件的 `formatTime`（相对时间）混淆

### D4: SessionDetail 拿配置不另起 reactive subscribe

**候选**：
- A: SessionDetail 已有 `config = $state(...)` props，`ftime` 闭包内直接读 `config.display.timeFormat === "12h"`
- B: 用 Svelte 5 `$derived` 派生一个 `hour12` 信号，`ftime` 依赖该派生

**选 A**。理由：
- `config` 本身就是 reactive `$state`，闭包内读 `config.display.timeFormat` 自动响应
- B 增加一层 indirection 无收益；`$derived` 适合"多个 state 合成"，此处单字段读取直读更清晰
- SettingsView `updateDisplay("timeFormat", "12h")` 改写 config 后，`config` 引用更新 → SessionDetail 渲染重跑 → `ftime` 重算 → 时间戳即时切换

## Risks / Trade-offs

- **[Risk] 历史用户体感突变**：12h → 24h 是用户可见行为变更，没有数据保证多数用户偏好哪个 → **Mitigation**：(a) release notes 显式标"默认改 24h，可在设置→显示→时间格式切回"；(b) SettingsView 提供 select 切换，30 秒内可改回
- **[Risk] enum 拼写漂移**：前后端 `"24h"` / `"12h"` 字符串字面散落两处，typo 难发现 → **Mitigation**：(a) `ipc_contract` 测试覆盖默认值 + 改写 + 非法值拒绝；(b) UI `api.ts` 定义 `type TimeFormat = "24h" | "12h"`，SettingsView select option 直接走该类型
- **[Risk] `formatClock` 单测覆盖不足**：依赖 `toLocaleTimeString` 浏览器 API 在 Node 测试环境的行为差异 → **Mitigation**：Vitest 跑时显式设 `process.env.TZ = "Asia/Shanghai"` 或断言用相对匹配（含/不含 "AM/PM"），不死锁具体小时数
- **[Trade-off] 不做 `"system"` 跟随系统选项**：少一个开关但避免 `Intl.DateTimeFormat` 多 locale 边界 bug。若日后呼声大可加（D1 的 enum 形式天然兼容）

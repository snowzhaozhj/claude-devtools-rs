## Why

`SessionDetail.svelte` 当前硬编码 `hour12: true` 显示"上午 X 点"形式的时间戳，且没有任何用户开关。中国大陆桌面 OS / IDE / 终端普遍采用 24 小时制，"上午/下午"前缀既占水平空间又与外部上下文割裂；同时本仓 `DisplayConfig` 已暴露 `showTimestamps` / `compactMode` 等用户偏好但唯独缺时间格式控制。本 change 把时间格式接入配置体系并把默认改为 24 小时制，让 UI 默认值与目标用户群体的桌面惯例对齐。

## What Changes

- 在 `DisplayConfig` 新增 `timeFormat: "24h" | "12h"` 枚举字段，默认 **`"24h"`** —— **BREAKING（user-visible 默认值变更）**：历史用户重启后看到 `SessionDetail` 时间戳从 "上午 X 点" 变成 `14:23:05`
- `ConfigManager::update_display` 加 `timeFormat` 白名单分支，非法字符串走 validation error，与现有 `theme` / `sessionClickBehavior` 等 enum 字段同模式
- 已有配置文件解析时 `display.timeFormat` 字段缺失 → 落到 `"24h"`（向后兼容序列化）
- `ui/src/lib/formatters.ts` 新增 `formatClock(date, hour12)` 入口，统一 `hh:mm:ss` 渲染；`SessionDetail.svelte` 把内联 `ftime()` 改走该入口
- `SettingsView.svelte` Display 区段加 enum select：「时间格式 · 24 小时制 / 12 小时制」
- `ipc_contract.rs` 加 `update_config_display_time_format_round_trip` 测试

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `configuration-management`: `DisplayConfig` 新增 `timeFormat` 字段；IPC `getConfig` / `updateConfig` 暴露该字段；默认值与字段验证规则纳入 spec

## Impact

- **Rust**：`crates/cdt-config/src/types.rs`（`DisplayConfig` + 新 `TimeFormat` enum）、`crates/cdt-config/src/manager.rs`（`update_display` 分支）、`crates/cdt-api/tests/ipc_contract.rs`（round-trip + 默认值）
- **UI**：`ui/src/lib/api.ts`（`DisplayConfig` interface 加字段）、`ui/src/lib/formatters.ts`（新 `formatClock`）、`ui/src/lib/__fixtures__/config.ts`（fixture 默认值同步为 `"24h"`）、`ui/src/routes/SessionDetail.svelte`（`ftime` 改走 `formatClock` + 从 config 派生 `hour12`）、`ui/src/routes/SettingsView.svelte`（Display 区段加 select）
- **测试**：Rust ipc_contract round-trip；Vitest 单测 `formatClock` 12h/24h 行为
- **向后兼容**：旧配置文件 `display` 段缺 `timeFormat` 时自动落 `"24h"`，不破坏序列化；旧前端 / 旧后端组合下双向均能正确退化
- **依赖**：无新 crate / npm 依赖

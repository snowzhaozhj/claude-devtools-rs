# Design: config-version-frontend

## Decisions

### D1: version 注入层选择 — cdt-api IPC/HTTP 层注入，不改 AppConfig struct

**选**：在 `LocalDataApi::get_config()` 返回时，序列化 `AppConfig` 后注入 `_version` 字段到 JSON object。

**弃选**：给 `AppConfig` struct 加 `version` 字段 — 会改 serde schema、影响 `update_config` 反序列化、需改所有消费 AppConfig 的地方。

**理由**：`_version` 是 transport metadata 非 domain data；`_` 前缀表示"非配置本身"，与已有 `extract_version` 的 `_version` 前缀一致。

### D2: 前端 version 存储 — SettingsView 组件内 state

**选**：`SettingsView.svelte` 内部 `let configVersion: number | null = $state(null)` 跟随 config 生命周期。

**弃选**：新建全局 `configStore.svelte.ts` — 目前仅 SettingsView 需要 version，全局 store 过度设计。

**理由**：遵循现有模式（config 已是组件 local state）；若未来需全局 store 再抽取。

### D3: 冲突提示方式 — toast + 自动重刷

冲突时：
1. 弹 toast（error 级别，4s）提示"配置已被其他窗口修改，已重新加载"
2. 自动 `getConfig()` 刷新本地 state

**弃选**：inline saveError 展示 — 用户可能没注意到 saveError（在页面底部）；toast 更醒目且自动消失。

## Data flow

```
[SettingsView] --updateConfig(section, {_version, ...data})--> [api.ts]
    --> invoke("update_config", {section, configData: {_version, ...}})
    --> [Tauri cmd] --> [LocalDataApi::update_config]
    --> [ConfigManager::update_general/display/...] -- extract_version --> check_version
        |-- OK: commit_next_config --> return AppConfig (with new _version injected at API layer)
        |-- Err(mismatch): return error string containing "Config version mismatch"
    <-- [SettingsView] catches error:
        |-- contains "mismatch": toast + getConfig() refresh
        |-- other: saveError inline display
```

## Changes by file

| File | Change |
|---|---|
| `crates/cdt-api/src/ipc/local.rs` | `get_config()` 返回 `serde_json::Value` 并注入 `_version` |
| `crates/cdt-api/src/ipc/traits.rs` | trait 签名改为返回 `Value`（或保留 AppConfig + version tuple） |
| `crates/cdt-api/tests/ipc_contract.rs` | 新增 `_version` 字段存在 + u64 类型断言 |
| `src-tauri/src/lib.rs` | `get_config` command 直接透传 Value（已是 Value） |
| `ui/src/lib/api.ts` | `getConfig()` 返回含 `_version` 的对象；`updateConfig()` 接受可选 version 参数 |
| `ui/src/routes/SettingsView.svelte` | 存 version state、传 version 到 update、处理 mismatch error |

## Risks

- **trait 签名变更影响面**：`DataApi::get_config` 改返回类型会影响 HTTP route。缓解：HTTP route 也用 `Json(value)` 透传。
- **序列化开销**：每次 get_config 多一次 `to_value` + inject。实际 AppConfig 很小（< 5KB），忽略不计。

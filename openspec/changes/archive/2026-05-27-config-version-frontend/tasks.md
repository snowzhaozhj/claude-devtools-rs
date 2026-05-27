# Tasks: config-version-frontend

## 1. 后端注入 _version 到 get_config 返回

- [x] `LocalDataApi::get_config()` 改为返回 `serde_json::Value`，注入 `_version` 字段
- [x] `DataApi` trait 签名同步更新
- [x] HTTP route `get_config` 适配新返回类型
- [x] IPC contract test 新增 `_version` 断言

## 2. 前端 store + 发送 version

- [x] `api.ts`: `getConfig()` 提取 `_version` 并返回；`updateConfig()` 接受 version 参数注入
- [x] `SettingsView.svelte`: 存 `configVersion` state、传 version 到 updateConfig

## 3. 前端处理 mismatch

- [x] `SettingsView.svelte`: catch error 含 "mismatch" 时 toast + 重刷 config

## 4. TS 类型 narrow

- [x] `GeneralConfig` 的 theme/defaultTab/sessionClickBehavior narrow 到 string literal union

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

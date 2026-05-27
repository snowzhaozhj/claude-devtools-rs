# configuration-management spec delta: config-version-frontend

## ADDED Requirements

### Requirement: Optimistic concurrency control for config updates

系统 SHALL 在配置读取响应中附加版本号，在配置更新请求中接受版本号并做乐观并发检查，以防止多客户端并发写入导致静默覆盖。

#### Scenario: get_config returns version field

- **WHEN** 调用方请求当前配置
- **THEN** 返回的 JSON 顶层 SHALL 包含 `_version` 字段，值为 `u64` 类型
- **AND** `_version` 的值 SHALL 等于 `ConfigManager` 当前内部 version

#### Scenario: update_config with matching version succeeds

- **WHEN** 调用方发送 update 请求，`configData` 中携带 `_version` 等于服务端当前 version
- **THEN** 更新 SHALL 成功
- **AND** 返回的配置 SHALL 包含递增后的新 `_version`

#### Scenario: update_config with stale version fails

- **WHEN** 调用方发送 update 请求，`configData` 中携带 `_version` 小于服务端当前 version
- **THEN** 系统 SHALL 返回包含 "Config version mismatch" 的错误信息
- **AND** 配置 SHALL 未被修改

#### Scenario: update_config without version is backward-compatible

- **WHEN** 调用方发送 update 请求，`configData` 中不含 `_version` 字段
- **THEN** 系统 SHALL 跳过版本检查，正常处理更新
- **AND** 这保证旧客户端 / CLI 工具的向后兼容

#### Scenario: Frontend shows conflict toast on version mismatch

- **WHEN** 前端发送 update 请求被拒（version mismatch）
- **THEN** 前端 SHALL 弹出 error 级别 toast 提示用户
- **AND** 前端 SHALL 自动重新获取最新配置以同步本地状态

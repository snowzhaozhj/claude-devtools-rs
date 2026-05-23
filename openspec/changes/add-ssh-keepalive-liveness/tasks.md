# Tasks

## 1. session.rs 配置 keepalive

- [x] 1.1 加 `SSH_KEEPALIVE_INTERVAL: Duration = 15s` + `SSH_KEEPALIVE_MAX: usize = 3` 顶部常量（pub）
- [x] 1.2 加 `fn build_client_config() -> Arc<russh::client::Config>` helper：用 `russh::client::Config { keepalive_interval, keepalive_max, ..Default::default() }` 语法构造（保证其它字段从 default 继承）
- [x] 1.3 `connect_inner` 阶段 2 把 `Arc::new(client::Config::default())` 替换成 `build_client_config()`
- [x] 1.4 单测 `build_client_config_enables_keepalive`：返回的 `Config.keepalive_interval == Some(SSH_KEEPALIVE_INTERVAL)` 且 `Config.keepalive_max == SSH_KEEPALIVE_MAX`
- [x] 1.5 grep 验证：`grep -n "client::Config::default()" crates/cdt-ssh/src/session.rs` 在本 change apply 后 SHALL 返 0 行（确认 `connect_inner` 真的换成了 helper，不只测了 helper 本身）

## 2. spec delta + validate

- [x] 2.1 写 `openspec/changes/add-ssh-keepalive-liveness/specs/ssh-remote-context/spec.md`，新增 `Requirement: Keep SSH transport alive via russh keepalive` 与 3 条 Scenario
- [x] 2.2 `openspec validate add-ssh-keepalive-liveness --strict` 通过
- [x] 2.3 design.md 含 D1-D5 决策完成

## 3. 本地验证

- [x] 3.1 `cargo test -p cdt-ssh` 全过（含新单测）
- [x] 3.2 `cargo clippy -p cdt-ssh --all-targets -- -D warnings` 无 warning
- [ ] 3.3 `just preflight` 一把梭通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

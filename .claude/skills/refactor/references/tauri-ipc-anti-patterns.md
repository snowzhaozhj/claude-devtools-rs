# Tauri IPC 边界识别（仓特定）

本文 **唯一职责** 是识别"看似搬代码、实际碰了 IPC 契约边界"的伪 refactor——命中即在 finding 里标 `category: boundary-3-ipc-*`，对应 SKILL.md §2 boundary guard 第 3 类。

**不在本 catalog 范围**（其它 skill / reviewer 的本职）：
- 配置链一致性（tauri-config-reviewer）
- 通知 / 托盘 / 平台 API 行为（spec / windows-compat-reviewer）
- updater / 发版（release-runbook）
- Windows 跨平台兼容（windows-compat-reviewer）
- 桌面端 smoke / mockIPC 同步（e2e-http-verify / qa-engineer）

## IPC 契约 / payload schema 边界

| category | 何时命中 |
|---|---|
| `boundary-3-ipc-payload-large` | 改动让单次 IPC payload > 1 MB |
| `boundary-3-ipc-schema-drift` | Rust 端字段名 / 形状改动（即使前端 caller 看似无碍）|
| `boundary-3-ipc-snake-leak` | 漏 `#[serde(rename_all = "camelCase")]` 导致序列化形状变化 |
| `boundary-3-ipc-error-stringify` | `Result<T, String>` 让前端无法 typed-match 错误 |

命中其一即视为伪 refactor，需走 §2 4 条证据降级或保持 boundary 标记。

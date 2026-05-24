# Tauri IPC 边界识别（仓特定）

识别碰 IPC 契约边界的伪 refactor，命中即在 finding 里标 `category: boundary-3-ipc-*`，对应 SKILL.md §2 boundary guard 第 3 类。

## IPC 契约 / payload schema 边界

| category | 何时命中 |
|---|---|
| `boundary-3-ipc-payload-large` | 改动让单次 IPC payload > 1 MB |
| `boundary-3-ipc-schema-drift` | Rust 端字段名 / 形状改动（即使前端 caller 看似无碍）|
| `boundary-3-ipc-snake-leak` | 漏 `#[serde(rename_all = "camelCase")]` 导致序列化形状变化 |
| `boundary-3-ipc-error-stringify` | `Result<T, String>` 让前端无法 typed-match 错误 |

命中其一即视为伪 refactor，需走 §2 4 条证据降级或保持 boundary 标记。

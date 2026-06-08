## 1. FsMetadata 加 created 字段

- [x] 1.1 `cdt-fs/src/metadata.rs`：`FsMetadata` 加 `created: Option<SystemTime>` 字段 + `created_ms()` 方法（fallback 到 mtime）
- [x] 1.2 `cdt-fs/src/local.rs`：`fs_metadata_from_std` 从 `meta.created().ok()` 填充 `created`
- [x] 1.3 修复所有 `FsMetadata` 构造点的编译错误（SSH provider 等填 `created: None`）

## 2. Session / SessionSummary 加 created 字段

- [x] 2.1 `cdt-core/src/project.rs`：`Session` struct 加 `created: i64` 字段（`#[serde(default)]`）
- [x] 2.2 `cdt-discover/src/project_scanner.rs`：`SessionStat` 加 `created_ms`，`Session` 构造从 `FsMetadata.created_ms()` 填充
- [x] 2.3 `cdt-api/src/ipc/types.rs`：`SessionSummary` 加 `created: i64` 字段（`#[serde(default)]`）
- [x] 2.4 `cdt-api/src/ipc/local.rs`：三个 `SessionSummary` 构造点填充 `created`（行 1140、2512、3753 附近）

## 3. QueryFilter 区间交集过滤

- [x] 3.1 `cdt-query/src/filter.rs`：`until` 条件从 `s.timestamp <= until` 改为 `s.created <= until`

## 4. 前端类型同步

- [x] 4.1 `ui/src/lib/api.ts`：`SessionSummary` 接口加 `created: number`
- [x] 4.2 `ui/src/lib/tauriMock.ts`：mock 数据构造加 `created` 字段

## 5. 测试

- [x] 5.1 `cdt-fs`：`created_ms()` 单测（Some 路径 + None fallback 路径）
- [x] 5.2 `cdt-query`：`QueryFilter` 区间交集过滤测试（跨午夜 session、完全超出范围 session、since-only、until-only）
- [x] 5.3 `cdt-api/tests/ipc_contract.rs`：`SessionSummary` round-trip 加 `created` 字段验证

## 6. 发布

- [ ] 6.1 push 分支 + 开 PR
- [ ] 6.2 wait-ci 全绿
- [ ] 6.3 codex + pr-review-toolkit 二审通过
- [ ] 6.4 archive change

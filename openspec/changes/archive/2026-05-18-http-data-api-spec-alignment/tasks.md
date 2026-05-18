## 1. Spec delta 落地（capability `http-data-api` → archive 时 sync 回 `openspec/specs/http-data-api/spec.md`）

- [x] 1.1 写 `openspec/changes/http-data-api-spec-alignment/specs/http-data-api/spec.md` 的 `MODIFIED Requirements` 段，覆盖 6 条 Requirement：`Serve projects and sessions over HTTP under /api prefix`、`Serve search endpoints`、`Serve auxiliary, subagent, utility, and validation endpoints`、`Serve config and notification endpoints`、`Push events via Server-Sent Events`、`Return safe defaults on lookup failures (current baseline)`
- [x] 1.2 在 `Serve search endpoints` Requirement 内显式把 URL 修正为 `POST /api/search`（**不是** `POST /api/search/sessions`），并在 Scenario 同步
- [x] 1.3 在 `Return safe defaults on lookup failures (current baseline)` Requirement 内列出完整 5 条 `code` × HTTP status 映射（`validation_error/400`、`config_error/400`、`not_found/404`、`ssh_error/502`、`internal/500`），并补 `PATCH config 非法值` + `SSH connect failure` 两条 Scenario
- [x] 1.4 在 `Push events via Server-Sent Events` Requirement 内显式写明 SSE 端点路径 `GET /api/events`，并把所有 SSE Scenario 的 "客户端已连接" 改为 "客户端连接 `GET /api/events`"
- [x] 1.5 不动 `Bind to configured port with graceful fallback` Requirement（无变化）

## 2. 校验

- [x] 2.1 跑 `openspec validate http-data-api-spec-alignment --strict`，必须 0 error / 0 warning 通过
- [x] 2.2 人工核对 spec delta 与 `crates/cdt-api/src/http/routes.rs::build_router` 的路由清单一致（25+ 路由全覆盖；没有写但 `routes.rs` 缺的项）
- [x] 2.3 人工核对 spec delta 错误码表与 `crates/cdt-api/src/ipc/error.rs::ApiErrorCode` enum + `routes.rs::IntoResponse for ApiError` 的映射完全一致
- [x] 2.4 跑 `cargo test -p cdt-api --test http_session_detail_global_lookup` 与 `cargo test -p cdt-api routes::tests`，确认现有测试继续通过（不应 break，因为本 change 不改代码）

## 3. followups 同步

- [x] 3.1 在 `openspec/followups.md::http-data-api [spec-gap] 路由前缀与错误码全部与实现偏差` 标 ✅ 并补 "已在 change `http-data-api-spec-alignment` 修正"

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（`openspec archive http-data-api-spec-alignment -y` + push 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

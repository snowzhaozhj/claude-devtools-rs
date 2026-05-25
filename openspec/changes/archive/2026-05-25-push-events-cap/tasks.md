## 1. 新建 push-events 主 spec delta（ADDED capability）
- [ ] 1.1 写 push-events spec delta（ADDED Requirements）：PushEvent enum 形态 + file-change / session-metadata-update / detected-error / sse-lagged / ssh-status-change variant payload 形态
- [ ] 1.2 直 edit push-events 主 spec Purpose 段（archive 后替换 TBD skeleton）

## 2. 修改 ipc-data-api spec delta（MODIFIED）
- [ ] 2.1 移走 `Emit push events for file changes and notifications` Requirement 内 file-change payload schema 整段 SHALL + 6 Scenario → 改为 transport 桥契约引用 `[[push-events]]`

## 3. 修改 http-data-api spec delta（MODIFIED）
- [ ] 3.1 SSE PushEvent payload 形态引用 `[[push-events]]`；保留 HTTP transport 层细节

## 4. 修改 frontend-test-pyramid spec delta（MODIFIED）
- [ ] 4.1 mockIPC listen event 名单来源改为引用 `[[push-events]]`

## 5. 修改 notification-triggers spec delta（MODIFIED）
- [ ] 5.1 `FileSignature` 自然恢复机制引用 `[[push-events::file-change]]`

## 6. 修改 sidebar-navigation spec delta（MODIFIED）
- [ ] 6.1 字段定义直接复制处改为引用 `[[push-events::file-change]]` / `[[push-events::session-metadata-update]]`；消费行为断言保留

## 7. 修改 session-display spec delta（MODIFIED）
- [ ] 7.1 同 sidebar-navigation 处理

## 8. baseline + validate
- [ ] 8.1 刷新 `scripts/spec-purity-baseline.txt` propose 期行
- [ ] 8.2 `openspec validate push-events-cap --strict` 通过
- [ ] 8.3 验收 grep 5 条全过

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

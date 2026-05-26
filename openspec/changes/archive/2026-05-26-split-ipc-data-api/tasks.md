## 1. spec delta 验证与修正

- [ ] 1.1 运行 `openspec validate split-ipc-data-api --strict` 确认 REMOVED 标题与主 spec 完全匹配
- [ ] 1.2 确认 ADDED Requirements 标题字符级匹配原 ipc-data-api 中的标题
- [ ] 1.3 行数校验：迁出行数 + 留下行数 ≈ 原始 2518（±Purpose 段差异）

## 2. 跨 spec 引用更新

- [ ] 2.1 grep 所有 `[[ipc-data-api::` 引用，检查是否指向被迁 Requirement，需更新为新 cap
- [ ] 2.2 grep 所有描述性引用（"ipc-data-api capability 的 ..."），确认语义仍正确

## 3. 最终验证

- [ ] 3.1 `openspec validate split-ipc-data-api --strict` 全绿
- [ ] 3.2 `openspec validate --all --strict` 确认其他 cap 不受影响

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

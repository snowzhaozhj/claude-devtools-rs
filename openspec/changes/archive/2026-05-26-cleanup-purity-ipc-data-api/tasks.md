# Tasks: cleanup-purity-ipc-data-api

## 1. Spec delta 清理
- [x] 1.1 重写 Purpose 段（去 `LocalDataApi`）
- [x] 1.2 重写 "Expose project and session queries" Requirement（最大命中源：lines 17-76）
- [x] 1.3 重写 "Lazy load inline image asset" 局部（line 228）
- [x] 1.4 重写 "Session 列表序列化暴露 cwd 字段" 局部
- [x] 1.5 重写 "get_session_detail 本地路径以单文件 stat 取元数据" 局部
- [x] 1.6 重写 "Contract test asserts..." 局部
- [x] 1.7 重写 "ProjectScanner shared read semaphore injection"
- [x] 1.8 重写 "ProjectScanCache 按事件语义分级失效" 局部
- [x] 1.9 重写 "SessionDetail 与高频 DataApi 方法..." 全文
- [x] 1.10 重写 "SessionDetailMetrics 与..." 全文
- [x] 1.11 重写 "ipc_contract 测试..." 全文
- [x] 1.12 重写 "Unified invalidator..." 局部

## 2. 验证
- [ ] 2.1 `openspec validate cleanup-purity-ipc-data-api --strict`
- [ ] 2.2 `bash scripts/check-spec-purity.sh --report` → ipc-data-api total = 0

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

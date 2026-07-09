## 1. 实现结构化递归脱敏

- [x] 1.1 `redact.rs`：新增对 `serde_json::Value` 的递归脱敏方法，对 `Value::String` 叶子值**和对象 key 字符串**跑 secret 正则替换（Array 递归元素，其余原样），返回 `(redacted_value, count)`
- [x] 1.2 `redact.rs`：`password` 正则保持原 `\S+`（apply 阶段撤销收窄，见 design D2b——D1 已保证结构安全，收窄会欠脱敏）
- [x] 1.3 `mcp/mod.rs`：emit 核心抽为 `redact_and_serialize(&Redactor, value)`（`to_string`→`from_str`→递归脱敏→`count>0` 包 `{data, redacted, redactedCount}` 否则原样返回原串），`emit_json` 调用它；删除「序列化→正则→from_str 失败回退」旧链路

## 2. 测试

- [x] 2.1 `redact.rs`：新增测试——含 `password=xxx` 且后接其他字段的结构化响应，脱敏后仍是合法 JSON、其余字段完整、命中被替换为 `[REDACTED]`
- [x] 2.2 `redact.rs`：新增测试——嵌套对象 / 数组元素内的 secret 也被递归脱敏；`redactedCount` 按叶子匹配总数正确累加
- [x] 2.3 `redact.rs`：新增测试——对象 **key 内**的 secret 也被脱敏（覆盖 `ToolOutputView::Structured.value` 保留任意 JSON 对象的场景，codex 二审 finding）
- [x] 2.4 保留既有 6 个纯字符串测试语义（迁移到新 API 或保留底层字符串脱敏辅助）；`cargo test -p cdt-cli` 全绿
- [x] 2.5 `mcp/mod.rs`：新增 emit 层单测（`redact_and_serialize`）——命中 secret 后响应仍是合法 JSON + 包裹 + 兄弟字段存活 / 无 secret 快路径不包裹 / 禁用(=allow-sensitive)透传（覆盖 #596 病灶层，pr-test-analyzer Gap1）

## 3. 收尾

- [x] 3.1 `CHANGELOG.md` `## [Unreleased]` 的 `### Fixed` 加一行（英文，面向用户）
- [x] 3.2 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all` + `openspec validate mcp-redact-preserve-json-structure --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR（PR body 链 Closes #596）
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

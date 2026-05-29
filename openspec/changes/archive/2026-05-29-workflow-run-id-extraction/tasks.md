## 1. cdt-core: ToolExecution 加字段

- [ ] 1.1 在 `crates/cdt-core/src/tool_execution.rs` 的 `ToolExecution` struct 加 `workflow_run_id: Option<String>` + `#[serde(default, skip_serializing_if = "Option::is_none")]`（camelCase 序列化为 `workflowRunId`）
- [ ] 1.2 grep 全部 `ToolExecution {` 构造点（预期 ~9 处），确认新字段 Option + serde(default) 不破坏编译

## 2. cdt-analyze: pair.rs 抽取 runId

- [ ] 2.1 在 `crates/cdt-analyze/src/tool_linking/pair.rs` 配对完成时，当 `pending.tool_name == "Workflow"` 且 `tool_use_result.get("runId")` 为 Some(string) 时写入 `workflow_run_id`
- [ ] 2.2 为 pair 单元测试加 fixture：含 Workflow tool_use + toolUseResult 有 runId / 无 runId / 非 string runId 三种 case

## 3. cdt-api: IPC contract test

- [ ] 3.1 在 `crates/cdt-api/tests/ipc_contract.rs` 加 round-trip test：Workflow ToolExecution 序列化含 `workflowRunId`；非 Workflow 不含该字段

## 4. 验证

- [ ] 4.1 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [ ] 4.2 `cargo test --workspace` 通过
- [ ] 4.3 `openspec validate workflow-run-id-extraction --strict` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

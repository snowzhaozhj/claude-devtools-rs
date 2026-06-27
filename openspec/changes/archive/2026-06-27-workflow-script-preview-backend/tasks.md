## 1. cdt-core：WorkflowItem 加字段

- [x] 1.1 `crates/cdt-core/src/workflow.rs`：`WorkflowItem` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub script_preview: Option<String>`
- [x] 1.2 `pending()` 构造器补 `script_preview: None`
- [x] 1.3 更新本文件内 `workflow_item_pending_roundtrip` / `workflow_item_full_roundtrip` 测试（断言 None 省略、Some 序列化为 `scriptPreview`）

## 2. cdt-api：填充 scriptPreview

- [x] 2.1 `workflow_manifest.rs` 顶部加 `const SCRIPT_PREVIEW_MAX_BYTES: usize = 32 * 1024` + `const MAX_SCRIPT_READ_BYTES: usize = 1 MB` + `truncate_script_preview(content: &str) -> String`（UTF-8 边界截断 + 尾部 marker 含原始字节数）+ oversize marker helper
- [x] 2.2 `read_script_meta` 改名 `read_script_data` 返回 `ScriptData { meta, preview }`，从同一次 read 派生 meta + 截断 preview；读前看 `fs_meta.size` > MAX_SCRIPT_READ_BYTES 则不全读、preview 仅 oversize marker（codex #7）；`ScriptCacheEntry`/`get_script`/`insert_script` 改缓存 `ScriptData`
- [x] 2.3 `resolve_running_state` 改用 `read_script_data` 取 `.meta`（name/phases 行为不变）
- [x] 2.4 `collect_workflow_candidates` 扩展携带 inline script（`exec.input.get("script")`）——改返回类型或加 struct
- [x] 2.5 `resolve_workflow_items` + `resolve_single`/`resolve_single_detail` 透传 inline_script；`resolve_single` 拆 `resolve_single_inner` + wrapper 单点设 `script_preview = resolve_script_preview(...)`
- [x] 2.6 `resolve_script_preview(script_path, inline_script, fs, cache)`：inline 优先（零 I/O 截断）→ scriptPath（`read_script_data().preview`）→ None

## 3. 测试

- [x] 3.1 `workflow_manifest.rs` 单测：inline 形态填 preview（零 I/O，断言不读文件）/ scriptPath 形态读文件填 preview / 超 32 KB 截断含 marker + UTF-8 边界 / 无来源 None / 缓存复用不重读
- [x] 3.2 `crates/cdt-api/tests/ipc_contract.rs`：`scriptPreview` 序列化 round-trip（Some present + None omitted）；更新现有 `WorkflowItem {` 字面构造点补字段
- [x] 3.3 `cargo test -p cdt-core -p cdt-api` 全绿；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --all`

## 4. 收尾验证

- [x] 4.1 `openspec validate workflow-script-preview-backend --strict`
- [x] 4.2 CHANGELOG `## [Unreleased] / ### Added` 加一行（用户可感知：workflow card 现可查看实际编排脚本）
- [x] 4.3 确认前端零改动（`api.ts` scriptPreview 字段、fixture、WorkflowCard disclosure 已就绪）
- [x] 4.4 记录 D6 已知限制（detail 路径 preview=None，仅影响 running workflow 轮询）——PR 描述说明，必要时开后续 issue

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

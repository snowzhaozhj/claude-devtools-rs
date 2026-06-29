# Tasks

## 1. 依赖与常量
- [x] 1.1 `cdt-api/Cargo.toml` 已含 `lru = { workspace = true }`（无需新增）
- [x] 1.2 `workflow_manifest.rs` 顶部加三个 cache 的 count cap / byte cap 常量（`ENTRIES_*` / `JOURNAL_*` / `SCRIPT_*`）

## 2. 改造 WorkflowManifestCache struct
- [x] 2.1 三个字段 `HashMap<PathBuf, *Entry>` → `lru::LruCache<PathBuf, *Entry>` + 各自 `*_bytes: usize` / `*_max_bytes: usize`
- [x] 2.2 `new()` 用默认配额构造（保持无参签名不变）；加 `#[cfg(test)]` 或 `with_caps` 测试构造器注入小配额
- [x] 2.3 三个 `estimate_*_bytes` 函数（CacheEntry / JournalCacheEntry / ScriptCacheEntry）

## 3. get/insert 改记账（签名不变）
- [x] 3.1 `get` / `get_journal` / `get_script`：peek 判签名 → 命中 `get`(bump) / mismatch `pop`(扣减) → 返回
- [x] 3.2 `insert` / `insert_journal` / `insert_script`：`push` + byte 记账 + while 淘汰至 ≤ max_bytes（保留 ≥1 条）

## 4. 测试（每 SHALL 一个用例）
- [x] 4.1 count cap 超限 LRU 淘汰（三 cache 各一，或参数化）+ 命中 bump 不被错误淘汰
- [x] 4.2 byte cap 超限 LRU 淘汰 + 单条超 cap 仍保留 1 条
- [x] 4.3 签名 mismatch 移除条目时字节计数扣减归零
- [x] 4.4 三个 cache 各自独立配额互不挤占（一个淘汰不影响另两个）
- [x] 4.5 既有 `cache_hit_and_miss` / `read_script_data_reuses_cache_no_double_read` 等回归仍过
- [x] 4.6 （pr-test-analyzer 加固）journal/script mismatch-pop 各自扣减字节 + entries 估算含 agents/phases

## 5. 验证
- [x] 5.1 `cargo clippy -p cdt-api --all-targets -- -D warnings`
- [x] 5.2 `cargo fmt --all`
- [x] 5.3 `cargo test -p cdt-api`（含 ipc_contract，确认无 IPC 字段回归）
- [x] 5.4 `openspec validate workflow-manifest-cache-byte-cap --strict`

## N. 发布
- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

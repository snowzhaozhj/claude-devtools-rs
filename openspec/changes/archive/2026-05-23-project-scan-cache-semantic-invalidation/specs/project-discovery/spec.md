## ADDED Requirements

### Requirement: `extract_session_cwd` 仅读首行的不变量

`cdt_discover::project_scanner::extract_session_cwd(jsonl_path)` 在解析 session JSONL 抽取 `cwd` 字段时，SHALL 在 JSONL 首行（第 1 行）即命中 `cwd` 字段并返回；MUST NOT 走 `read_to_string` 整文件兜底分支。

**为何此不变量重要**：`change project-scan-cache-semantic-invalidation` 的 `ProjectScanCache` 失效语义依赖此前提——已知 session 的 JSONL 追加（第 21+ 行写入）SHALL NOT 改变 `extract_session_cwd` 抽取结果。若 claude-code 未来引入"先建空 jsonl 再补 cwd"或"cwd 在中后段"的格式，本不变量会被破坏，需要先在此 capability 重新评估抽取语义并对应调整 `ipc-data-api::ProjectScanCache 按事件语义分级失效` 的失效粒度。

**实现现状**：当前 `extract_session_cwd` 实现为「读首 20 行，逐行尝试解析为 user message 取 `cwd` 字段；20 行内未命中且为本地路径时 fallback `read_to_string` 整文件再扫」（参见 `crates/cdt-discover/src/project_scanner.rs:328-405`）。本 Requirement 不改实现行为；只把"首行命中"从隐含行为升级为**契约**，由测试守护。

**测试断言机制**：测试 SHALL 用 `cdt_fs::with_fs_counter` 包裹 `extract_session_cwd` 调用并使用其返回值（`FsOpCounts` snapshot）做断言；不能仅靠返回值（cwd）断言（cwd 正确不代表未走兜底，可能首行 + 兜底都命中得到同一 cwd）。`FsOpCounter::snapshot()` 是 crate 私有方法，禁止直接调用——`with_fs_counter` 内部计算并把 `FsOpCounts` 作为返回 tuple 第二项暴露给调用方（`crates/cdt-fs/src/instrumentation.rs:122-145`）。

**测试构造 fs 的硬要求**：scanner 内部调用的 `FileSystemProvider` 必须用 `cdt_fs::InstrumentedFs::new(...)` 包装才能让 `FsOpCounter` 实际计数。未包 wrapper 的 provider 调 trait 方法不递增 counter（向后兼容设计，详 `instrumentation.rs:153-160`）。`ProjectScanner::new(projects_dir, fs)` / `new_with_semaphore(projects_dir, fs, semaphore)` 都接受 `Arc<dyn FileSystemProvider>`，测试 SHALL 传入 `Arc::new(cdt_fs::InstrumentedFs::new(cdt_fs::local_handle()))`。

#### Scenario: 首行含 cwd 时 SHALL 不触发 `read_to_string` 兜底

- **WHEN** 测试用 `tempfile::tempdir` + `tokio::fs::write` 构造一个 1000 行的 session JSONL：第 1 行为含 `"cwd": "/path/to/proj"` 字段的合法 user message JSON；其余 999 行为不含 `cwd` 的 assistant message JSON
- **AND** 测试构造 `let fs = Arc::new(cdt_fs::InstrumentedFs::new(cdt_fs::local_handle()));` 并据此构造 `ProjectScanner`
- **AND** 测试调用 `let (cwd, counts) = cdt_fs::with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;`
- **THEN** `cwd` MUST 等于 `Some("/path/to/proj".to_string())`（与首行字面量一致）
- **AND** `counts.read_to_string` MUST == 0（兜底分支未触发）

#### Scenario: 已有首行 cwd 时 JSONL 后续追加 SHALL NOT 改变抽取结果

- **WHEN** 测试构造 JSONL 仅含 1 行 user message + cwd，scanner fs 同上 wrapper
- **AND** 调 `with_fs_counter(|| async { scanner.extract_session_cwd(...).await }).await` 拿到 `(R1, counts1)`
- **AND** 用 `tokio::fs::OpenOptions::append` 在该 JSONL 末尾追加 100 行不含 cwd 的 assistant message
- **AND** 再次调用 `with_fs_counter(|| async { scanner.extract_session_cwd(...).await }).await` 拿到 `(R2, counts2)`
- **THEN** R1 MUST == R2
- **AND** `counts1.read_to_string` 和 `counts2.read_to_string` MUST 都 == 0

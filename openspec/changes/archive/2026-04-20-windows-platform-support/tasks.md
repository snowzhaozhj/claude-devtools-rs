## 1. cdt-discover: 路径编解码对齐 TS 原版

- [x] 1.1 `crates/cdt-discover/src/path_decoder.rs::home_dir()` 改为四级 fallback：`HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir()`
- [x] 1.2 `decode_path` 扩展识别三种格式：legacy `C--Users-...`、新 Windows `-C:-Users-...`（解出后不加 POSIX 前缀）、POSIX `-Users-...`；保留 WSL mount 转换（仅 `cfg(target_os = "windows")`）
- [x] 1.3 新增 `pub fn encode_path(absolute_path: &str) -> String`：同时替换 `/` 与 `\` 为 `-`，保留盘符冒号，强制 leading `-`；导出到 `lib.rs`
- [x] 1.4 `is_valid_encoded_path` 认 legacy 格式 `^[A-Za-z]--[A-Za-z0-9_.\s-]+$`
- [x] 1.5 单元测试：Windows round-trip / legacy 解码 / `is_valid_encoded_path` 认 legacy / `encode_path` 双分隔符 / WSL mount（仅 Windows `cfg`）
- [x] 1.6 `resolve_home_dir` 纯函数版 + env mock 注入测试（绕过 `forbid(unsafe_code)` 对 `env::set_var` 的禁用）
- [x] 1.7 `cargo clippy -p cdt-discover --all-targets -- -D warnings` 通过
- [x] 1.8 `cargo fmt --all`
- [x] 1.9 `cargo test -p cdt-discover` 全过（21 passed）

## 2. cdt-config: 收敛 encode_path + 企业路径动态取

- [x] 2.1 `crates/cdt-config/Cargo.toml` 加 `cdt-discover = { workspace = true }` dep
- [x] 2.2 `crates/cdt-config/src/claude_md.rs` 删除私有 `encode_path`，改 `use cdt_discover::encode_path`
- [x] 2.3 `enterprise_path()` 的 Windows 分支改用 `std::env::var("ProgramFiles")` 动态取，fallback `C:\Program Files`；保留 macOS / Linux 分支不变
- [x] 2.4 替换旧 `encode_path_*` 测试为 `auto_memory_encoded_dir_uses_cdt_discover_encoder`（期望值修正为含 leading `-`，旧断言本身就错）
- [x] 2.5 `cargo clippy -p cdt-config --all-targets -- -D warnings` 通过
- [x] 2.6 `cargo fmt --all`
- [x] 2.7 `cargo test -p cdt-config` 全过（99 passed）

## 3. cdt-watch: canonicalize + project_id 跨平台

- [x] 3.1 workspace root `Cargo.toml` 加 `dunce = "1"` workspace dep
- [x] 3.2 `crates/cdt-watch/Cargo.toml` 引入 `dunce = { workspace = true }`
- [x] 3.3 `FileWatcher::with_paths` 中 `projects_dir.canonicalize()` / `todos_dir.canonicalize()` 切 `dunce::canonicalize`
- [x] 3.4 `parse_project_event` 的 `components.iter().map(...).join("/")` 改为 `components.iter().collect::<PathBuf>().to_string_lossy().into_owned()`
- [x] 3.5 `cargo clippy -p cdt-watch --all-targets -- -D warnings` 通过
- [x] 3.6 `cargo fmt --all`
- [x] 3.7 `cargo test -p cdt-watch -- --test-threads=1` 全过（6/6）

## 4. cdt-api: asset URL 路径归一

- [x] 4.1 `crates/cdt-api/src/ipc/local.rs::materialize_image_asset`：`file_path.display()` → `file_path.to_string_lossy().replace('\\', "/")`
- [x] 4.2 `crates/cdt-api/tests/perf_get_session_detail.rs::projects_dir` 的 `env::var("HOME")` 替换为 `cdt_discover::get_projects_base_path()`（复用统一 home 解析）
- [~] 4.3 单元测试 `materialize_image_asset_generates_forward_slash_url` —— 函数内部混合 sha2 hash + 写盘，抽离成本高；归一逻辑是一行 `replace`，复查即可，不单独补测
- [x] 4.4 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过
- [x] 4.5 `cargo fmt --all`
- [x] 4.6 `cargo test -p cdt-api` 全过

## 5. cdt-config & cdt-ssh: expand_tilde 双前缀 + 敏感路径黑名单

- [x] 5.1 `crates/cdt-config/src/mention.rs::validate_file_path` 的 tilde 展开：`trim_start_matches(['/', '\\'])` 覆盖 `~/` 与 `~\`
- [x] 5.2 `crates/cdt-config/src/mention.rs::SENSITIVE_PATTERNS` 追加 Windows 特有条目：`config\SAM` / `config\SYSTEM` / `NTDS.dit` / `Credentials` / `Crypto` / `Protect`（全大小写不敏感）
- [x] 5.3 `crates/cdt-ssh/src/config_parser.rs::expand_tilde` 同步：`strip_prefix('~')` + `trim_start_matches(['/', '\\'])`；保留 `~username` 不展开
- [x] 5.4 单元测试：`mention.rs::windows_sensitive_paths_blocked`；`config_parser.rs::expand_tilde_supports_forward_and_backslash` + `expand_tilde_without_separator_keeps_original`
- [x] 5.5 `cargo clippy -p cdt-config -p cdt-ssh --all-targets -- -D warnings` 通过
- [x] 5.6 `cargo fmt --all`
- [x] 5.7 `cargo test -p cdt-config -p cdt-ssh` 全过

## 6. Tauri 配置 + CI Matrix

- [x] 6.1 `src-tauri/tauri.conf.json::build.beforeDevCommand` 改为 `npm run dev --prefix ../ui`（跨 shell 兼容）
- [x] 6.2 `.github/workflows/ci.yml` 的 clippy / test 两个 job 统一 matrix：`os: [ubuntu-latest, windows-latest, macos-14]`；`fail-fast: false`
- [x] 6.3 fmt job 保持 `ubuntu-latest` 单跑（rustfmt 平台无关）
- [x] 6.4 clippy / test job 在三平台都跑
- [ ] 6.5 git commit + push 分支，等 Windows runner 实际绿 —— 本地无法验证，留到 PR CI

## 7. 质量复查 fix（Phase 6 reviewer 反馈）

- [x] 7.1 **Blocker**：`crates/cdt-api/tests/agent_configs.rs` 私有 `encode_path` 副本违反 "单一 encode 源" 承诺且只处理 `/`，Windows CI 必 fail —— 删副本，改 `use cdt_discover::encode_path`
- [x] 7.2 **Warning**：`path_decoder::is_valid_encoded_path` legacy 分支加字符集校验（`[A-Za-z0-9_.\s-]+`，对齐 TS 原版 regex + spec 要求），拒绝非 ASCII rest 段
- [x] 7.3 **Warning**：`cdt-discover::path_decoder::home_dir` 升为 `pub fn` + 导出；`cdt-config::claude_md::claude_base_path` / `mention::claude_base_path` / `mention::validate_file_path` 三处改调 `cdt_discover::home_dir`，与 `get_projects_base_path` 行为一致（Windows `HOMEDRIVE+HOMEPATH` fallback 场景也能正确解析）
- [x] 7.4 **Warning**：`mention::validate_file_path` 的 tilde 展开加 `~user` 保护（仅 `~` / `~/` / `~\` 展开；`~alice/foo` 保留原样），对齐 `cdt-ssh::config_parser::expand_tilde`
- [x] 7.5 新增测试：`is_valid_encoded_path` legacy 边界（非 ASCII / `/` 字符 / 数字盘符）；`mention::tilde_user_form_not_expanded`
- [x] 7.6 跳过：`fallback_home()` 加 tracing warn（nit）、`cfg!` vs `#[cfg]` 统一（风格）

## 8. preflight + openspec validate

- [x] 8.1 `just fmt`
- [x] 8.2 `just lint`（workspace + src-tauri 独立 manifest）
- [x] 8.3 `cargo test --workspace --exclude cdt-watch` 全过 + `cargo test -p cdt-watch -- --test-threads=1` 6/6 过
- [x] 8.4 `just check-ui`（0 errors，5 pre-existing warnings）
- [x] 8.5 `just spec-validate`（21/21 passed，含本 change）

## 9. Archive + 提交与 PR

- [ ] 9.1 `openspec archive windows-platform-support -y` —— 同 PR 内归档，reviewer 审查时直接看最终主 spec 状态（方案 A）
- [ ] 9.2 commit 业务改动 + archive 产物（两个原子 commit）
- [ ] 9.3 push 分支 + `gh pr create`，PR body 引用 proposal + 三个 P0 修复点
- [ ] 9.4 等 CI 全绿（重点看 `windows-latest` 三 job）后 request review
- [ ] 9.5 若 review 要求改 spec delta：`git revert` archive commit → 改 delta → 重新 archive → force-push

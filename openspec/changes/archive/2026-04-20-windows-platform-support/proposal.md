## Why

Rust port 首版 release `v0.1.0` 在 Windows 上能启动但 **session 列表为空**，无法正常使用。经排查有以下硬伤：

1. `crates/cdt-discover/src/path_decoder.rs::home_dir()` 自实现，**只读 `HOME` 环境变量**。Windows native 上 `HOME` 通常不存在，fallback 到 `PathBuf::from("/")`，`get_projects_base_path()` 返回当前盘符根下的 `\.claude\projects`，数据目录永远找不到。
2. `crates/cdt-discover/src/path_decoder.rs::decode_path` 强行为解码结果加 POSIX leading `/`，Windows 上的 `C:\Users\...` 编码目录无法正确还原；也不识别 TS 原版的 legacy 格式 `C--Users-...`。
3. `crates/cdt-config/src/claude_md.rs::encode_path` 只替换 `/` 不替换 `\`，Windows auto-memory 路径计算失败。
4. `crates/cdt-api/src/ipc/local.rs::materialize_image_asset` 用 `format!("asset://localhost/{}", file_path.display())`，Windows 上 `display()` 产生含 `\` 的字符串，Tauri webview 不识别该 URL，内联图片加载失败。
5. `crates/cdt-watch/src/watcher.rs::with_paths` 用 `projects_dir.canonicalize()`，Windows 下返回 `\\?\C:\...` UNC 前缀，与 `notify` 回调传回的普通路径前缀不匹配，`path.starts_with(&self.projects_dir)` 永远 false，file-change 实时刷新完全失效。
6. `crates/cdt-config/src/mention.rs` 与 `crates/cdt-ssh/src/config_parser.rs::expand_tilde` 只匹配 `~/`，Windows SSH config 里的 `~\` 不展开；敏感路径黑名单也缺少 Windows 条目（`SAM` / `NTDS.dit` / `Credentials`）。
7. `.github/workflows/ci.yml` 只跑 `ubuntu-latest`，Windows 路径从未经过 fmt / clippy / test 验证 —— 所以上述 bug 全部在 CI 层未被拦截。

TS 原版（`claude-devtools/src/main/utils/pathDecoder.ts`）已正确处理 Windows 所有细节（四级 home fallback、legacy `C--` 格式、WSL `/mnt/c/` 转换、`[/\\]` 双分隔符 regex），Rust port 在简化移植时丢了这些分支。

## What Changes

- **MODIFIED**：`project-discovery` capability 的 `Scan Claude projects directory` Requirement —— 明确 Windows 上 home 目录解析顺序为 `HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir()`，保证能定位到 `%USERPROFILE%\.claude\projects\`。
- **MODIFIED**：`project-discovery` capability 的 `Decode encoded project paths` Requirement —— 新增三种编码格式识别（新格式 `-C:-Users-...`、legacy 格式 `C--Users-...`、POSIX 格式 `-Users-...`）与 WSL mount 路径转换（仅 Windows），对齐 TS 原版行为。
- **ADDED**：`project-discovery` capability 新增 `Encode absolute paths into directory names` Requirement —— 把"把 `/` 和 `\` 都替换为 `-`、保留盘符冒号、强制加 leading `-`"的规则作为正式契约固定下来，并成为 `cdt-discover::path_decoder::encode_path()` 的唯一实现源（`cdt-config::claude_md::encode_path` 删除私有副本，改 import）。
- 非 spec 改动：
  - `cdt-api::ipc::local::materialize_image_asset` 生成 `asset://` URL 时把 `\` 归一为 `/`。
  - `cdt-watch::FileWatcher` 引入 `dunce` crate 去除 Windows `\\?\` UNC 前缀；`parse_project_event` 不再用 `join("/")`。
  - `cdt-config::mention` 与 `cdt-ssh::config_parser` 的 `expand_tilde` 识别 `~/` 与 `~\` 两种前缀；`mention` 敏感路径黑名单追加 Windows 特有项。
  - `src-tauri/tauri.conf.json::beforeDevCommand` 改为跨 shell 兼容的 `npm run dev --prefix ../ui`。
  - `crates/cdt-api/tests/perf_get_session_detail.rs` 把裸 `env::var("HOME")` 替换为 `dirs::home_dir()`。
  - `.github/workflows/ci.yml` 的 fmt / clippy / test 三个 job 扩展 matrix 到 `[ubuntu-latest, windows-latest, macos-14]`，把 Windows 路径纳入回归保护。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `project-discovery`：home 目录解析 + decode_path 识别 Windows 格式 + 新增 encode_path Requirement。

## Impact

- **代码**
  - `crates/cdt-discover/src/path_decoder.rs`：重写 `home_dir()`；扩展 `decode_path`；新增 `pub fn encode_path`；更新 `is_valid_encoded_path` 认 legacy；补单元测试。
  - `crates/cdt-config/src/claude_md.rs`：删除私有 `encode_path`，改 `use cdt_discover::path_decoder::encode_path`；`enterprise_path` Windows 分支改用 `env::var("ProgramFiles")` 动态取。
  - `crates/cdt-config/Cargo.toml`：加 `cdt-discover` workspace dep（当前未依赖）。
  - `crates/cdt-watch/src/watcher.rs`：`canonicalize` 切 `dunce::canonicalize`；`parse_project_event` 保留 OS 原生分隔符。
  - `crates/cdt-watch/Cargo.toml` + workspace root：新增 `dunce = "1"` workspace dep。
  - `crates/cdt-api/src/ipc/local.rs`：`asset://` URL 构造归一 `\` → `/`。
  - `crates/cdt-config/src/mention.rs`：`expand_tilde` 双前缀匹配；敏感路径 regex 追加 Windows 条目。
  - `crates/cdt-ssh/src/config_parser.rs`：`expand_tilde` 双前缀匹配。
  - `crates/cdt-api/tests/perf_get_session_detail.rs`：`env::var("HOME")` → `dirs::home_dir()`。
  - `src-tauri/tauri.conf.json`：`beforeDevCommand` 跨 shell 兼容。
  - `.github/workflows/ci.yml`：三 job matrix 扩展到 Windows + macOS。
- **依赖**：新增 `dunce = "1"` workspace dep（仅 `cdt-watch` 使用；零运行时开销、无传递依赖）。
- **测试**：
  - `path_decoder`：Windows 路径 round-trip（`C:\a\b\c` ↔ `-C:-a-b-c`）、legacy `C--Users-...` 解码、WSL `/mnt/c/` 转换（仅 `cfg(target_os = "windows")` 断言具体转换结果）、`HOME` 不存在时 `USERPROFILE` 与 `HOMEDRIVE+HOMEPATH` fallback。
  - `claude_md`：现有测试保留，仅 import 路径改动；`enterprise_path` 新测试覆盖 `ProgramFiles` env 读取。
  - CI：新加 Windows runner 需要 cache（`Swatinem/rust-cache` 已对 Windows 兼容）；`cdt-watch` 的 FSEvents-specific 测试在 Windows 不 flaky，不额外 exclude。
- **回滚**：本 change 无性能/行为开关，回滚等同 `git revert`。TS 原版 Windows 行为是 v0.1.0 首次引入（之前从未支持），回滚只是恢复"Windows 上不可用"状态。
- **后续**：WSL distro 扫描、Windows Authenticode 签名、Windows 路径大小写不敏感比较、`tauri.conf.json::bundle.windows` 配置 —— 独立 change（见 `openspec/followups.md` 预留条目）。

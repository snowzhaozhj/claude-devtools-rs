# Design: Windows Platform Support

## Context

首版 `v0.1.0` release workflow 把 Windows 纳入 matrix 打包，但从未在 Windows 环境跑过 fmt / clippy / test，也无人实机验证启动流程。用户反馈 Windows 上应用能启动但 session 列表为空。交叉探查代码库后定位到 Rust port 在移植 TS 原版 `claude-devtools` 时丢掉了多处 Windows 分支。

## Goals

- 修复 Windows 上 session 列表为空、图片不显示、文件监听失效三大硬伤。
- 把路径编解码收敛到 `cdt-discover::path_decoder` 单一源，消除当前散落在 `cdt-discover` 与 `cdt-config` 两处的分叉实现。
- 在 CI 层建立 Windows 回归保护，防止未来再次出现"本地改得好、到 Windows 崩"的回归。
- 行为对齐 TS 原版 `claude-devtools/src/main/utils/pathDecoder.ts`，为未来可能的 cross-port 复查保留一致契约。

## Non-Goals

- **不**做 Windows 上 WSL distro 扫描（TS 原版有，但属独立功能，单开 change）。
- **不**配置 `tauri.conf.json::bundle.windows` 签名 / Authenticode（需要证书，分发层面的问题）。
- **不**做 Windows 路径大小写不敏感比较（TS 原版有，但对现有场景影响小）。
- **不**改 `decode_path` 对老数据格式的兼容性语义 —— 新增识别 legacy 格式只是多认，不拒绝老格式。

## Decisions

### D1: 共用的 `encode_path` 放在 `cdt-discover::path_decoder`

**Alternatives**:
- (a) 放 `cdt-core`（零 runtime crate，纯数据）
- (b) 放 `cdt-discover::path_decoder`（当前 `decode_path` / `extract_base_dir` 都在这里）
- (c) 各 crate 保留私有副本（现状）

**Decision**: (b)。理由：
- `encode_path` 与 `decode_path` / `is_valid_encoded_path` 在语义上互为反操作，应同文件定义、同份测试覆盖。放 `cdt-discover` 让 "项目路径编解码" 成为一个聚合点。
- `cdt-core` 是"类型定义 + trait 契约"的 crate，新加函数会稀释其定位。
- (c) 是现状，已经造成 bug（两副本分叉），不保留。
- `cdt-config` 目前不依赖 `cdt-discover`；加 dep 后依赖方向是 `cdt-config → cdt-discover → cdt-core`，无环。

### D2: `home_dir()` 的 fallback 顺序对齐 TS

**Order**: `HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir()`

**Alternatives**:
- (a) 直接 `dirs::home_dir()`（跨平台 crate，已经处理了 Windows `USERPROFILE`）
- (b) 对齐 TS 原版的四级显式 fallback

**Decision**: (b)。理由：
- TS 原版优先 `HOME` 的行为允许 WSL / Git Bash / Cygwin 用户通过 env 显式覆盖 —— `dirs::home_dir()` 在 Windows 上**不**读 `HOME`。
- 用户若在 Git Bash 下启动 Tauri app（虽少见），环境继承 `HOME` 的行为需要保留。
- 四级 fallback 也让 "`USERPROFILE` 被意外清空" 的极端场景仍能通过 `HOMEDRIVE+HOMEPATH` 兜底。
- 代码量小（约 10 行），维护成本低。

### D3: `decode_path` 三种格式分支识别

**Parser 流程**（对齐 TS `decodePath`）：
```rust
// 1. Legacy 格式 `C--Users-alice-app`（无 leading `-`，冒号编码为 `--`）
if let Some((drive, rest)) = encoded.strip_match(r"^([A-Za-z])--(.+)$") {
    return format!("{drive_upper}:/{rest_slashed}");
}

// 2. 去掉 leading `-`
let trimmed = encoded.strip_prefix('-').unwrap_or(encoded);
let slashed = trimmed.replace('-', "/");

// 3. 新格式 `C:/Users/alice/app` - 直接返回，不加 POSIX 前缀
if slashed starts_with /^[A-Za-z]:\//  { return slashed; }

// 4. POSIX 格式 - 补前缀
let posix = if slashed.starts_with('/') { slashed } else { format!("/{slashed}") };

// 5. WSL mount 转换（仅 Windows）
#[cfg(target_os = "windows")]
translate_wsl_mount(&posix)  // /mnt/c/code → C:/code
```

**依据**：TS 原版已经这么做，且测试覆盖完整（round-trip `C:/Users/alice/app` ↔ `-C:-Users-alice-app`；legacy `C--Users-alice-app` → `C:/Users/alice/app`）。对齐之后 Windows session 的 `cwd` 展示、path resolver fallback 全部正确。

### D4: `FileWatcher` 的 `canonicalize` 用 `dunce`

**Problem**：Windows `std::fs::canonicalize` 返回 `\\?\C:\Users\...` UNC 形式，`notify` crate 回调传入的路径是普通 `C:\Users\...`，`starts_with` 匹配失败。

**Decision**：引入 `dunce = "1"` crate（11 KB 纯 Rust、零依赖、MIT license、1.8k stars），`dunce::canonicalize` 在非 UNC 路径上去掉 `\\?\` 前缀；macOS / Linux 行为与 `std::fs::canonicalize` 一致，不引入新风险。

**Alternatives rejected**：
- 自实现 strip `\\?\`：要处理 `\\?\UNC\server\share\...` 合法 UNC 路径不能 strip 的边界，写对不容易。
- `std::path::PathBuf::strip_prefix` 双向尝试：隐晦，维护成本高。

### D5: `parse_project_event` 不用 `join("/")`

**Before**:
```rust
let project_id = components[..components.len() - 1]
    .iter()
    .map(|c| c.as_os_str().to_string_lossy())
    .collect::<Vec<_>>()
    .join("/");
```

**After**:
```rust
let project_id = components[..components.len() - 1]
    .iter()
    .collect::<PathBuf>()
    .to_string_lossy()
    .into_owned();
```

**Rationale**：`project_id` 作为 IPC 负载字段，消费端不解析它做文件系统拼接 —— 只把它作为不透明 key 匹配 `ProjectScanner` 返回的 encoded 目录名。因此保留 OS 原生分隔符即可（Unix `/`、Windows `\`），下游匹配按字符串相等即可。之前硬 join `/` 在 Windows 上会产生 `-C:-Users-xxx\subdir` 式的奇怪字符串，与 scanner 输出不匹配。

### D6: CI Matrix 加 Windows + macOS

**Before**: 三个 job（fmt / clippy / test）全部 `ubuntu-latest`。

**After**: matrix `[ubuntu-latest, windows-latest, macos-14]`。

**Trade-off**：
- CI 时间 3x —— `Swatinem/rust-cache` 对三平台都有效，增量 build 下每平台 ~2 min，绝对值可接受。
- `cdt-watch` 的 FSEvents 测试在 Windows 不触发（因为是 ReadDirectoryChangesW）—— 不用 exclude。
- Linux runner 仍是主 gating job（最快、dep 最全），fail 不必等其他平台。

## Testing Strategy

### 单元测试（`cdt-discover/src/path_decoder.rs`）

- `windows_path_round_trip`：`encode_path("C:\\Users\\alice\\app")` → `-C:-Users-alice-app`；`decode_path` 还原为 `C:/Users/alice/app`。
- `legacy_windows_format_decodes`：`decode_path("C--Users-alice-app")` → `C:/Users/alice/app`。
- `is_valid_encoded_path_recognizes_legacy`：`"C--Users-foo"` → true；`"C:-..."`（无 leading `-`，新格式但缺前缀）→ false。
- `encode_replaces_both_separators`：`encode_path("C:\\a/b\\c")` → `-C:-a-b-c`。
- `home_dir_prefers_home_over_userprofile`：env isolation 下 `HOME=/foo` + `USERPROFILE=C:\bar` → `/foo`。
- `home_dir_falls_back_to_userprofile`：`HOME` unset + `USERPROFILE=C:\bar` → `C:\bar`。
- `home_dir_falls_back_to_homedrive_homepath`：仅 `HOMEDRIVE=C:` + `HOMEPATH=\Users\alice` → `C:\Users\alice`。

这些测试跨平台能跑（不依赖真实 FS）；`env` 修改用 `std::env::set_var` + 测试互斥（`serial_test`）或独立 subprocess 隔离。

### `cdt-config::claude_md` 现有测试不变

`encode_path_replaces_slashes` 等继续过 —— 函数签名没变，只是实现转到 `cdt-discover`。Windows 新分支用 `cfg(target_os = "windows")` + `#[cfg(test)]` 隔离。

### 集成

- `cdt-watch` `file_watching.rs` 已有 FSEvents flake 标记。Windows 上 `notify` 走 ReadDirectoryChangesW 完全不同路径，本 change 不动测试本身，只验证 CI matrix 绿不绿。
- Tauri UI 层面不加测试 —— `asset://` URL 归一是一行字符串替换，无测试价值。

### CI 验证

本 change 自身在 Windows CI 上跑完整 `cargo test --workspace --locked` 就是验证 —— merge 前看 Windows runner 绿。

## Rollback

本 change 无开关（不是性能优化，是正确性修复），回滚 = `git revert`。Windows 恢复"能启动但 session 列表空"状态。不影响 macOS / Linux。

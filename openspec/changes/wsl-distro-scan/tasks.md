## 1. cdt-discover：WSL 枚举模块

- [ ] 1.1 新建 `crates/cdt-discover/src/wsl.rs`，定义 `pub struct WslDistroCandidate { distro: String, home_path: String, claude_root_path: String, claude_root_exists: bool }` 与 `pub struct WslDistroScanReport { candidates: Vec<WslDistroCandidate>, distros_without_home: Vec<String> }`，serde 派生（`rename_all = "camelCase"`）
- [ ] 1.2 实现 `pub async fn list_distros() -> Result<WslDistroScanReport>`：
  - 非 Windows 直接返回 `{ candidates: [], distros_without_home: [] }`
  - Windows 调三段命令 fallback（`--list --quiet` / `-l -q` / `-l`），每段对 3 个 wsl.exe 路径 candidate 依次尝试（System32 / Sysnative / PATH），4s timeout / 调用
  - 任一组合解析出非空 distro 列表立即跳到 home 解析阶段
  - 用 `tokio::process::Command` 异步 spawn；用 `futures::future::join_all` 并发解析每个 distro 的 home（5s timeout / distro）
- [ ] 1.3 实现 `decode_wsl_output(bytes: &[u8]) -> String` 私有函数：BOM 检测 → heuristic（前 ≤512 字节奇数 index NUL 比例 ≥ 30%）→ UTF-16 LE 解码；否则 UTF-8 lossy 解码；解码后**全局** strip 所有 `\0`；8 个边界 case 单测（spec scenario 同步覆盖：含 BOM / heuristic 命中 UTF-16 / 纯 ASCII / 仅 BOM / 奇数总字节 / 行末 NUL / 行内 NUL / 混合 `\r\n`）
- [ ] 1.4 实现 `parse_wsl_distros(stdout: &str) -> Vec<String>` 私有函数：按 `\r?\n` 切行 → trim + strip NUL → 过滤头部说明行（`startsWith("windows subsystem for linux")` / `contains("default version")` / `startsWith("the following is a list")`，小写比较）→ 去前缀 `*\s*` → 去后缀 `(default)`（大小写不敏感）→ 去重（小写比较）；4 个边界 case 单测（含说明行 / `*` 前缀 / `(Default)` 后缀 / 重复 distro）
- [ ] 1.5 实现 `normalize_wsl_home_path(input: &str) -> Option<String>` 私有函数：trim → 必须以 `/` 开头 → posix normalize（解 `..` / 折叠多 `/`）→ 去尾随 `/`（除非整路径就是 `/`）；3 个边界 case 单测
- [ ] 1.6 实现 `resolve_home(distro: &str) -> Option<String>` 私有函数：跑 `wsl -d <distro> -- sh -lc 'printf %s "$HOME"'` (5s timeout) → `decode_wsl_output` → `normalize_wsl_home_path` → `None` 时 fallback 到 `std::env::var("USERNAME")` 拼 `/home/<USERNAME>` 再 normalize；fallback 仍 `None` 时返回 `None`
- [ ] 1.7 实现 `build_unc_path(distro: &str, home: &str) -> String` 私有函数：`/` → `\` 转换、拼 `\\wsl.localhost\<distro>\...\\.claude`；2 个边界 case 单测（含连字符 distro 名）
- [ ] 1.8 错误模型按 design D8 落地：返回 `WslDistroScanReport` 结构；spawn `NotFound` / 三段命令全失败 / 解析后空 → `{ candidates: [], distros_without_home: [] }`；部分 distro 解 home 失败 → 进 `distros_without_home`；全部 distro 解 home 失败 → `{ candidates: [], distros_without_home: [...全部 distro 名] }`；其他 io 错误 → `Err`
- [ ] 1.9 候选 sort：`candidates` 按 `distro` 名升序排列
- [ ] 1.10 `crates/cdt-discover/src/lib.rs` 导出 `pub mod wsl;` 与 `pub use wsl::{WslDistroCandidate, WslDistroScanReport};`
- [ ] 1.11 `cargo test -p cdt-discover wsl::` 全绿；`cargo clippy -p cdt-discover --all-targets -- -D warnings` 全绿

## 2. cdt-api：IPC command

- [ ] 2.1 在 `crates/cdt-api/src/ipc/` 找到现有 mod 入口（参考 `list_repository_groups` 等命令位置），新增 `list_wsl_distros` async fn handler
- [ ] 2.2 handler 调用 `cdt_discover::wsl::list_distros()`，返回 `WslDistroScanReport`；error map 到 `IpcError`
- [ ] 2.3 在 `cdt-api` IPC contract test（`tests/ipc_contract.rs` 或同源）加 case 验 `WslDistroCandidate` + `WslDistroScanReport` 序列化字段名（camelCase: `distro` / `homePath` / `claudeRootPath` / `claudeRootExists` / `candidates` / `distrosWithoutHome`）
- [ ] 2.4 `cargo test -p cdt-api --test ipc_contract` 全绿

## 3. src-tauri：注册命令 + 授权

- [ ] 3.1 `src-tauri/src/lib.rs::invoke_handler!` 注册 `list_wsl_distros`
- [ ] 3.2 `src-tauri/capabilities/default.json` 加授权（按 Tauri 2 默认 cmd 授权约定）
- [ ] 3.3 `src-tauri/Cargo.toml` 检查 `cdt-api` feature 是否需要新 feature flag（不需要新 flag —— 命令在所有平台返回单形态）
- [ ] 3.4 `cd src-tauri && cargo check --release` 全绿（macOS）

## 4. ui：共享 Modal 组件 + Settings "Use WSL" 按钮

- [ ] 4.1 新建 `ui/src/lib/components/Modal.svelte`：最小可用通用 dialog 组件，含 title slot / content slot / 主按钮（label / variant / disabled props）/ 取消按钮 / ESC 关闭 / 点击遮罩关闭 / a11y 焦点陷阱 + `aria-modal="true"` + `role="dialog"`；样式与现有 `Settings*.svelte` 视觉一致
- [ ] 4.2 在 `ui/src/routes/SettingsView.svelte` General section 找到 `claudeRootPath` 输入控件位置，紧邻下方加 "Use WSL" 按钮（`<button>` 元素，复用现有 settings button 样式 class）
- [ ] 4.3 按钮 visibility 用 `is_windows()` 判定：优先复用 `app-chrome` 或现有平台判定 IPC；如无则用 `navigator.userAgent.includes("Windows")` 作 fallback；非 Windows 平台**不渲染**按钮 DOM
- [ ] 4.4 点击 handler：`invoke('list_wsl_distros')` → 按 design D6 + spec settings-ui 的 6 类 scenario 分支处理：
  - `candidates.length == 1` → 直接 `update_config`
  - `candidates.length >= 2` → 用 4.1 的 `Modal` 包裹 distro radio list 弹出
  - `candidates.length == 0 && distrosWithoutHome.length == 0` → inline 文案"未检测到 WSL distro"
  - `candidates.length == 0 && distrosWithoutHome.length > 0` → inline 文案"检测到 WSL distro 但无法解析 home（<distro 名列表>）"
  - IPC 调用失败 → inline 错误文案
- [ ] 4.5 distro 选择 modal radio list：每行展示 distro 名 + UNC 路径 + `claudeRootExists=false` 时附文案"该 distro 内尚无 Claude 数据"；不禁用任何选项；用户选定 + 点 "Apply" 主按钮后调 `update_config`
- [ ] 4.6 `update_config` 调用复用现有 SettingsView 的配置写入路径，确保乐观更新 + 失败回滚（与现有 settings 行为一致）

## 5. ui 测试

- [ ] 5.1 `ui/tests/` 加 vitest 用例：mockIPC `list_wsl_distros` 返回 5 类 case（单 candidate / 多 candidate / 空 + 空 / 空 + distrosWithoutHome 非空 / IPC 失败），验按钮点击后的 UI 行为对齐 spec settings-ui scenarios
- [ ] 5.2 `Modal.svelte` 加独立单测：ESC 关闭 / 遮罩点击关闭 / 主按钮触发 callback / a11y 属性 4 case
- [ ] 5.3 `pnpm --dir ui run test` 全绿
- [ ] 5.4 `pnpm --dir ui run check` 全绿（svelte-check）

## 6. 平台 smoke 验证

- [ ] 6.1 macOS 本地 `just dev` 启动，确认 Settings General section 不渲染 "Use WSL" 按钮（非 Windows）
- [ ] 6.2 macOS 本地直接 `invoke('list_wsl_distros')`（devtools console 或单测），确认返回 `{ candidates: [], distrosWithoutHome: [] }` 不报错
- [ ] 6.3 Windows 平台 smoke：发版前由有 Windows + WSL 环境的开发者手测，验证：① 多 distro 弹 modal；② 选定后 `claudeRootPath` 持久化；③ `claudeRootExists=false` 候选可选；④ 无 WSL 时空结果 inline 提示；⑤ 全 distro home 失败时 inline 提示文案区分

## 7. perf + spec validate 自检

- [ ] 7.1 perf 估算：N=5 distro 场景下 `list_wsl_distros` 端到端 wall 预算 < 6s（5s home timeout 主导，不在 hot path）；记入 PR 描述但不强制 4 维 baseline（非 hot path）
- [ ] 7.2 `openspec validate wsl-distro-scan --strict` 全绿
- [ ] 7.3 `just preflight` 全绿（fmt + lint + test + spec-validate）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

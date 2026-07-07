# Tasks: flexible-data-root

## 1. 后端 config：tilde 校验 + recentRoots

- [x] 1.1 `cdt-config/src/validation.rs`：`validate_claude_root_path` 在 `looks_like_absolute_path` 前接受 `~/`（Windows `~\`）前缀（保留原形不展开），拒绝 `~user/` 具名 home 与相对路径
- [x] 1.2 `cdt-config/src/types.rs`：`GeneralConfig` 新增 `recent_roots: Vec<String>`（serde camelCase `recentRoots`，`#[serde(default)]` 向后兼容）
- [x] 1.3 `cdt-config/src/manager.rs`：写入非 `null` `claudeRootPath` 时更新 `recentRoots`（去重键=规范化字符串〔trim 尾分隔符 + Windows 大小写不敏感〕 + MRU 排序 + 上限，默认上限 8）；append 走既有版本化 `update_config` 事务不旁路（D7）；加载与写入时过滤非法项并 warn 日志（F7，便于诊断损坏配置 / 手工编辑错误，UI 不弹错）；partial-merge 缺字段补空数组
- [x] 1.4 单测（`cdt-config`）：tilde 接受/拒绝矩阵（`~/x`、`~\x` 接受存原形、`~alice/x` 拒绝、`foo/bar` 拒绝、绝对路径接受）；recentRoots 去重键规范化/MRU/上限/空合并/非法项过滤

## 2. 后端 discovery：tilde 展开消费点

- [x] 2.1 `cdt-discover/src/path_decoder.rs`：抽统一 root 解析 helper（`~/` / `~\` 前缀展开，复用现有 home fallback 链，绝对路径原样），helper 落 `cdt-discover` **不反向依赖 `cdt-ssh`**（F6）
- [x] 2.2 三个 root 消费点全部改走该 helper（F1 + 复验第三点）：`projects_base_path_for`、`todos_base_path_for`、`cdt-api/src/ipc/local.rs::claude_base_path`（被 `read_claude_md_files` / context annotation 消费 CLAUDE.md + memory）
- [x] 2.3 确认 watcher `todos_dir` 从同一 resolved root 派生；`jobs_dir` 保持固定 `~/.claude/jobs` 不随数据根切换（背景任务目录 [[background-jobs]]，非会话数据）
- [x] 2.4 单测（`cdt-discover` / `cdt-api`）：`~/.qoder` → projects / todos / claude_base 三处均 `<home>/.qoder/...`；`~\.qoder` 等价；绝对路径不动；`~alice/` 不展开

## 3. CLI：--root 全局覆盖参数

- [x] 3.1 `cdt-cli/src/main.rs`：新增全局 `--root` / `--data-dir` 参数，支持 `~/` 展开
- [x] 3.2 按优先级链 `--root` > `config.claudeRootPath` > 默认解析出 resolved root，**同时贯穿 `build_local_data_api()` 与 `run_serve()` 两条构造路径**（F2：serve 的 HTTP/watcher/SSE 也用 resolved root，不回退 config）；`--root` **不写回 `claudeRootPath` / `recentRoots`**（F3，允许 load migration 的独立 side effect）
- [x] 3.3 `--root` 非法路径（相对 / `~user/`）以非零退出码 + 错误说明失败
- [x] 3.4 单测/集成测（`cdt-cli`）：`--root` 覆盖生效且 `claudeRootPath`/`recentRoots` 未被写入；`cdt --root ~/.qoder serve` 的 HTTP 取数走 Qoder 根（F2）；无 `--root` 继承配置；`--data-dir` 别名等价；非法路径报错
- [x] 3.5 回归测（F3）：磁盘配置含旧 composite project id 时运行 `cdt --root ~/.qoder ...`，load migration 可 persist，但 `--root` 值 SHALL NOT 进入待持久化 AppConfig（`claudeRootPath`/`recentRoots` 不含该 override）

## 4. 前端 settings：label 泛化 + MRU 下拉

- [x] 4.1 `ui/src/routes/SettingsView.svelte`：数据目录 label "Claude 数据根目录" → "数据根目录"（字段名不变）
- [x] 4.2 数据根快速切换控件：读 `general.recentRoots` 渲染历史下拉，选中经 `updateGeneral("claudeRootPath", ...)` 切换；`recentRoots` 为空时下拉隐藏/禁用，手输 + 选目录入口不变
- [x] 4.3 前端类型 + `api.ts`：`GeneralConfig` 增 `recentRoots`
- [x] 4.4 vitest 单测：MRU 下拉渲染 + 选中切换 + 空历史降级；label 文案

## 5. 契约测试与验证

- [x] 5.1 Rust IPC contract test（`cdt-api`）：`update_config("general", ...)` payload 含 `recentRoots` 字段形状
- [x] 5.2 `pnpm --dir ui run check` + `just test-ui-unit`
- [x] 5.3 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all`
- [x] 5.4 真实数据 e2e 验收：临时 `--root ~/.qoder` 跑 `cdt projects list` 看 Qoder 项目；桌面端 MRU 切换到 `~/.qoder` 后 sidebar 正常渲染（`e2e-http-verify` 或 `just dev` 手动 smoke）
- [x] 5.5 `openspec validate flexible-data-root --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

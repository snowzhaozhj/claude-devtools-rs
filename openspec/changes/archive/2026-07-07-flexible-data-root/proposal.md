## Why

公司要求从 Claude Code 迁移到 Qoder，存在一段需同时查看两边会话的过渡期（未来终局不确定）。已 spike 验证 Qoder 的 `~/.qoder/projects/<encoded>/<uuid>.jsonl` 与 Claude Code JSONL 三层同源（目录布局 / 字段 / chunk-turn 语义全兼容，Qoder 特有 type `runtime-config` / `last-prompt` / `ai-title` 被 parser 静默跳过无害，`cwd` 扫 head 20 行正常还原），现有 `cdt-parse` / `cdt-discover` 直接可读。

但切换到别的数据根目前有三个已核实根因的摩擦：(1) 手输 `~/.qoder` 被校验判为非绝对路径直接拒绝（`validation.rs` 的 `looks_like_absolute_path` 前无 `~/` 展开）；(2) 切换入口埋在 Settings 深处，只能手输或选目录，每次重复；(3) `cdt-cli` 虽已继承桌面端 `claudeRootPath` 配置，但无命令行覆盖，无法脱离共享配置临时指向别处。

本 change 把"数据根目录访问"做**通用化**改进以消除这三点摩擦，**不引入任何 Qoder 专属逻辑**——所有能力对任意数据根都成立，即便未来无人使用 Qoder 也不留技术债。

## What Changes

- **tilde 展开（通用）**：`claudeRootPath` 接受 `~/`（Windows `~\`）前缀。采用"存 tilde 原形、消费时展开"策略——config 会跨机器 / 跨平台同步，存 `~/.qoder` 比存展开后的 `/Users/xxx/.qoder` 可移植。展开集中在一个共享 helper，覆盖该 root 派生的**全部**消费路径（projects / todos / CLAUDE.md 与 auto-memory 读取），不遗漏任何一处。放宽 `configuration-management` 与 `project-discovery` 中"必须绝对路径"的既有口径为"绝对路径或 `~/` / `~\` 开头"。
- **CLI `--root` / `--data-dir`（通用）**：`cdt-cli` 新增全局参数，支持 tilde，与 GUI 同一套路径解析。语义为**临时覆盖、不持久化**——优先级链 `--root`（仅当次调用） > `config.claudeRootPath`（继承桌面端） > 默认 `~/.claude`。resolved root **SHALL** 贯穿该次调用全部消费路径（含 `cdt serve` 的 HTTP / watcher / SSE）；**MUST NOT** 因 `--root` 写入配置的 `claudeRootPath` 或 `recentRoots`（既有 load migration 的独立 side effect 不受此约束）。
- **数据根 MRU 历史（通用）**：`config.general` 新增 `recentRoots: string[]`，记录用户切换过的数据根（任意路径都进历史，不扫盘、不硬编码任何目录名）。Settings 数据目录处提供下拉快切，消除"每次重输"摩擦。
- **label 泛化（通用）**：Settings 中 "Claude 数据根目录" 文案改为 "数据根目录"；底层字段名 `claudeRootPath` 保持不变（内部名不影响用户，零 breaking）。

**明确不做**（留门给未来演进，避免过度投入 / Qoder 绑定）：自动扫 home 检测数据源；sidebar 一等公民源切换器；`config.sources[]` 多源 schema；Qoder token 统计适配（Qoder assistant 无 `message.usage`，属 Qoder 专属，违背不绑原则）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `configuration-management`: `claudeRootPath` 校验放宽为接受 `~/` 前缀（存原形，消费时展开）；新增 `general.recentRoots: string[]` 字段的持久化 / 加载 / 合并契约。
- `project-discovery`: 当前 Claude root 解析补充 `~/` / `~\` 前缀展开语义，经统一 helper 覆盖 projects / todos / CLAUDE.md 全部 root 派生路径。
- `cli-output`: CLI 命令结构新增全局 `--root` / `--data-dir` 参数，定义其"临时覆盖不持久化"语义、优先级链与 serve 路径贯穿。
- `settings-ui`: General Section 数据目录子块新增 MRU 下拉快切交互；label 文案泛化。

## Impact

- **代码**：`cdt-config/src/validation.rs`（tilde 接受 + `recentRoots` 校验）、`cdt-config/src/types.rs`（`recentRoots` 字段）、`cdt-config/src/manager.rs`（合并 / 持久化 / 去重）、`cdt-discover/src/path_decoder.rs`（统一 root 展开 helper）、`cdt-api/src/ipc/local.rs`（`claude_base_path` 等三消费点过 helper + serve resolved root）、`cdt-cli/src/main.rs`（全局 arg + 覆盖优先级贯穿 serve）、`ui/src/routes/SettingsView.svelte`（MRU 下拉 + label）。
- **配置格式**：`claude-devtools-config.json` 的 `general` 段新增 `recentRoots` 字段（缺失时按默认合并，向后兼容）；`claudeRootPath` 允许存 `~/` 原形（旧的绝对路径值仍有效）。
- **IPC / 字段**：`update_config("general", ...)` payload 新增 `recentRoots`；需同步 IPC contract test。
- **依赖**：无新增；tilde 展开复用 `cdt-discover::home_dir()` 思路，不引入 `shellexpand`。
- **无 BREAKING**：字段名不变、旧配置值兼容、CLI 默认行为不变（仅新增可选覆盖）。

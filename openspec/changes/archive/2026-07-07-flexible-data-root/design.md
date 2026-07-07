## Context

应用已支持自定义数据根：`general.claudeRootPath` 为 `null` 时用默认 `~/.claude`，为绝对路径时整体换根（`cdt-discover::projects_base_path_for` 是分叉点）。桌面端 Settings 有输入框 + 文件选择器 + 恢复默认三入口，CLI 的 `ConfigManager::new(None)` 读同一份 `~/.claude/claude-devtools-config.json`，因此 CLI 已继承桌面端配置。

公司 Claude Code → Qoder 迁移进入过渡期，需低摩擦地在多个数据根间切换。spike 已确认 Qoder 数据与 Claude Code JSONL 三层同源、现有 parser/discovery 直接可读。当前三个摩擦点均已核实根因：(1) `validation.rs` 的 `looks_like_absolute_path` 前无 `~/` 展开，手输 `~/.qoder` 被拒；(2) 切换入口只能手输或选目录，重复劳动；(3) CLI 无命令行覆盖，无法脱离共享配置临时指向别处。

硬约束（来自用户）：链路轻、不强绑定 Qoder、通用化——即便未来无人用 Qoder 也不留技术债。因此所有产出均为通用能力，实现中不出现 "qoder" 字样。

## Goals / Non-Goals

**Goals:**
- 手输 `~/` 前缀数据根可用（GUI + CLI 同一解析语义）
- 切换数据根低摩擦（历史快速切换，免重复手输）
- CLI 可临时覆盖数据根而不污染桌面端共享配置
- 全部通用，零 Qoder 专属逻辑

**Non-Goals:**
- 自动扫 home 检测数据源（略重且有 Qoder 味）
- sidebar 一等公民源切换器 / `config.sources[]` 多源 schema（留未来 Level 2，终局明朗再上）
- Qoder token 统计适配（Qoder assistant 无 `message.usage`，属 Qoder 专属，违背不绑原则）
- 聚合/合并多源会话视图

## Decisions

### D1：tilde 存原形，消费时展开（不在持久化层展开为绝对路径）

`claudeRootPath = ~/.qoder` 持久化时保留 `~/` 原形，实际 home 展开推迟到数据读取消费点（`projects_base_path_for` 之前）。

- **候选 A（选中）存原形**：config 跨机器 / 跨用户 home 同步时可移植——同一份配置在不同机器解析到各自 home 下的目录。
- **候选 B 存展开后绝对路径**（`/Users/zhaohejie/.qoder`）：把当前机器的 home 烤死进配置，同步到别的机器/用户即失效。
- 取舍：本仓配置已有跨平台同步语义（terminalApp 跨平台降级即证），A 与既有设计一致。代价是展开点需集中管理（见 D4）。

### D2：MRU 历史替代"自动扫描检测源"

用 `general.recentRoots: string[]`（切换过的路径进历史）+ Settings 下拉快切，替代"主动扫 home 找 `*/projects/` 数据源"。

- **候选 A（选中）MRU 历史**：不扫盘、不硬编码任何目录名、任何路径都进历史——最轻、最通用、零 Qoder 绑定。WSL candidate 是"扫描发现"因为用户不知道 UNC 路径；本地路径用户知道（`~/.qoder`），只需"记住切过的"，无需主动扫描。
- **候选 B 自动检测源**：要定义扫描范围/深度/性能预算，且"检测 .claude/.qoder"隐含 Qoder 语义——违背不绑原则，未来无人用 Qoder 时沦为死代码。
- 取舍：A 解决"切换效率低"的成本远低于 B，且没有绑定债。`recentRoots` 的元素形态与 `claudeRootPath` 一致（`~/` 原形），未来若升格 `sources[]` 可平滑演进。

### D3：CLI `--root` 临时覆盖、不持久化（关键设计决策）

`--root`（别名 `--data-dir`）只作用于当次进程调用，**绝不写回配置文件**。优先级链：`--root` > `config.claudeRootPath` > 默认 root。

- **为什么不持久化**：CLI 与桌面端共享同一份 `claude-devtools-config.json`。若 `--root` 写回配置，`cdt --root ~/.qoder ls` 会顺带把桌面端也切走——破坏"CLI 看 Qoder、GUI 仍看 Claude"的并行使用（正是过渡期的典型诉求）。
- **候选 B（否决）`--root` 落盘**：等价于"CLI 改配置"，与桌面端切换耦合，制造隐蔽的跨进程副作用。
- 取舍：临时覆盖是纯运行时决策，`build_local_data_api` 在解析优先级链后构造 scanner，`--root` 值只读不写。
- **覆盖所有消费路径**（codex 二审 F2）：普通子命令走 `build_local_data_api()`，但 `cdt serve` 走 `run_serve()` 单独从 config 构造 projects/todos/watcher/HTTP。resolved root **SHALL** 应用于该次调用的**全部**数据根消费路径——两条构造路径都接受同一个 resolved root，不得只改 `build_local_data_api` 而漏 serve。
- **不变量措辞收紧**（codex 二审 F3）：不变量是"**不因 `--root` 写入 `claudeRootPath` 或 `recentRoots`**"，而非"配置文件一字节不变"——`ConfigManager::load()` 既有的 composite-id migration 仍可能独立触发 `persist_config`，那与 `--root` 无关，不违反本不变量。

### D4：抽统一 root 解析 helper，所有 root 消费点集中过它

home 展开集中在**一个**共享 helper（落 `cdt-discover`，例如 `resolve_claude_root(claude_root: Option<&str>) -> PathBuf`），config 持久化层与 CLI 参数解析层都只做"接受/校验 `~/`(`~\`) 形态"、不展开；**所有**读取数据根的消费点都经该 helper 拿到已展开的绝对 root，再各自拼子目录。

- **消费点全集**（codex 二审 F1 + 复验第三点）：`claudeRootPath` 派生出**三个** root 消费点，SHALL 全部经统一 helper——(a) `projects_base_path_for`（scanner + watcher projects）；(b) `todos_base_path_for`（watcher todos）；(c) `LocalDataApi::claude_base_path()`（local.rs:2348，被 `read_claude_md_files` / context annotation 消费 CLAUDE.md + auto-memory）。**首版 D4 只列 projects+todos 是漏的**——claude_base 直接 `PathBuf::from(claude_root)` 当字面路径，`~/.qoder` 会读错 CLAUDE.md/memory。抽 helper 后"唯一展开点"才名副其实。
- **jobs 明确不跟 root**：`FileWatcher` 的 `jobs_dir` 固定 `~/.claude/jobs`（watcher.rs:92），是 claude-devtools **自身后台任务队列**（[[background-jobs]] / [[file-watching]] 均固定 `~/.claude/jobs`），语义上不是"被查看的会话数据"，**SHALL NOT** 随数据根切换。
- **依赖方向**（codex 二审复验 F6）：tilde 展开 helper 落 `cdt-discover`（home 解析 fallback 链本就在此），**不**让 `cdt-config` / `cdt-discover` 反向依赖 `cdt-ssh`。`cdt-ssh::expand_tilde` 是私有实现，仅作双分隔符处理的参照，不直接复用其符号。
- `~/` 或 Windows `~\` 前缀（紧跟分隔符）才展开；`~user/` 具名 home 不展开（校验层已拒），与现有 `cdt-config::mention.rs` 对 `~user` 不展开的口径一致。

### D5：label 泛化但字段名不变（零 breaking）

UI 文案 "Claude 数据根目录" → "数据根目录"；底层配置字段名 `claudeRootPath`、Rust `claude_root_path` **保持不变**。

- 字段名是内部/IPC 协议标识，改名要 migration 且触碰 IPC contract；文案是纯展示层。只改文案即可去 Claude 化，代价最小、无兼容风险。

### D6：recentRoots 去重键 = 规范化字符串比较（codex 二审 F4）

MRU 去重不做文件系统 canonicalize（路径可能不存在、跨机器同步、需 IO），而按**规范化字符串**比较：trim 尾部 `/` `\`、Windows 上大小写不敏感、`~/` 与 `~\` 归一。

- **已知限制**：因 D1 存 `~/` 原形而文件选择器返回展开后的绝对路径，同一目录经"手输 `~/.qoder`"与"选择器选 `/Users/alice/.qoder`"可能落成两条历史项。这是存原形策略的必然代价，接受之——两条都能正确切换、只是历史里并列；不值得为消除它引入消费侧反向折叠（把绝对路径压回 `~/`）的复杂度。UI 可在候选项显示原始形态帮助用户辨识。
- 候选（否决）：真实 inode/canonicalize 去重——要 IO、路径不存在时失败、跨机器无意义。

### D7：recentRoots 写入走既有版本化事务；跨进程并发限制显式接受（codex 二审 F5 + 复验）

recentRoots 的 append 发生在后端处理 `update_general(claudeRootPath)` 的**同一次事务内**，基于当前配置态 merge 后整份持久化，走既有 `Optimistic concurrency control for config updates`（`_version` stale 检查）。约束：append **SHALL NOT** 走绕过版本检查的旁路 save。

- **同进程并发**（多 GUI client 同后端）：既有乐观锁足够——stale `_version` 被拒。**行为澄清**（复验修正）：现有前端 stale 处理是 `refreshAfterMismatch()` **拒绝并重取最新态、不自动重放**用户刚才的切换——即用户需重试一次，而非系统替他重写。首版 D7 写的"重取最新态再写"措辞不准，已纠正为"拒绝并重取，用户重试"。
- **跨进程并发限制**（复验揭示）：`_version` 是 **session-local**（每个 `ConfigManager` 实例从 0 起），而 `cdt serve` 是独立进程、HTTP `/api/config` PATCH 可写 config——桌面进程与 serve 进程各持一份 version，跨进程 **last-write-wins** 仍可能覆盖 recentRoots。**这是既有全局限制**（所有 config 字段皆然，非本 change 引入），recentRoots 不比别的字段更糟。本 change **明确不升级并发模型**（引入文件锁 / 写前重读磁盘 merge 属重改，违背"链路轻"约束）；接受此限制并在 Risk 记录，留未来若上多源持久化时统一解决。

## Visual Contract

本 change 的 UI 改动限于**现有** `SettingsView.svelte` 的 General → 数据目录子块，复用既有 `SettingsField` / `SettingsButton` / `control-input` 组件，**不新建 `.svelte` 文件、不改核心面板 ≥2、不加 Settings tab、不加 modal**——不触发 `opsx-apply-cadence` 的 impeccable 重构强制钩子。

- **Surface**：沿用当前数据目录子块位置，不新增入口。
- **新增控件**：数据根快速切换（MRU）——在现有输入框旁增加历史下拉；`recentRoots` 为空时下拉隐藏或禁用，手输 + 选目录两条既有入口保持不变（覆盖 empty 状态）。
- **State Coverage**：默认（`claudeRootPath=null`）/ 自定义值 / 历史为空 / 历史非空四态，均由 settings-ui spec 的 Scenario 兜底。
- 若 apply 阶段下拉的具体视觉形态（下拉 vs 分段按钮 vs combobox）需设计裁决，再按需 `/impeccable shape` 单点征询；本 change 默认判定为单点交互增强，不预调 impeccable（与"链路轻"约束一致）。

## Risks / Trade-offs

- **[Qoder token 统计不准]** → Qoder assistant 无 `message.usage`（用量在独立 `runtime-config` 行）。本 change 不适配，会话正文查看不受影响；如未来需要，独立开 change 给 context-tracking 加 adapter。已在 proposal 显式列为 Non-Goal。
- **[`recentRoots` 上限选值]** → 上限过大列表冗长、过小历史易丢。design 建议默认上限 8，apply 时可调；spec 只约束"有上限 + 去重 + MRU"语义，不锁死数字。
- **[`~/` 在 Windows 的语义]** → `~/` 是 POSIX 惯例；Windows 用户更可能用盘符绝对路径或 `~\`。D4/F6 已让 `~\` 等价 `~/`（复用 `cdt-ssh::expand_tilde` 双分隔符），展开复用现有 home fallback 链（含 `USERPROFILE`），不新增平台分支。
- **[CLI `--root` 与配置漂移]** → 用户可能困惑"为何 CLI 看到的和 GUI 不同"。缓解：`--root` 仅当次生效属预期语义；文档/help 文案说明其临时覆盖性质。
- **[`recentRoots` 含坏项]**（codex 二审 F7）→ 磁盘配置或 IPC payload 可能带非法项（相对路径 / `~user/x` / 非字符串）。缓解：加载与 append 时 SHALL 过滤非法项（沿用 `claudeRootPath` 同一套校验），坏项不进历史下拉，避免"点了才被拒"的死历史。

## Migration Plan

- 配置向后兼容：旧配置缺 `recentRoots` 字段时合并为空数组（走既有 partial-merge 路径）；旧的绝对路径 `claudeRootPath` 值仍有效。
- 无需数据迁移、无 schema 版本 bump。
- 回滚：字段与参数均为增量新增，回滚只需移除代码；已写入的 `recentRoots` 对旧版本是未知字段，按既有 partial-merge 容忍。

## Open Questions

- `recentRoots` 上限最终值（默认 8，apply 时依据实际手感定）。
- 快速切换控件的具体视觉形态（下拉 / combobox / 分段）——留 apply 阶段按 Visual Contract 决定，必要时单点征询 impeccable。

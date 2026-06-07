## MODIFIED Requirements

### Requirement: CLI 命令结构

CLI binary `cdt` SHALL 提供以下顶级命令结构：

- `cdt projects` — 列出所有项目
- `cdt sessions` — 列出 session（支持全局或按项目）
- `cdt session <id>` — 单 session 复合视图（summary + cost + errors）
- `cdt session <id> --chunks` — chunk 级内容取数
- `cdt search <query>` — 全文搜索
- `cdt stats [period]` — 聚合统计
- `cdt serve` — HTTP API server
- `cdt mcp serve` — MCP stdio server
- `cdt setup` — 安装配置
- `cdt completions <shell>` — shell 补全脚本生成
- `cdt self-update` — 自更新

`cdt session <id>` 和 `cdt session <id> --chunks` 共用同一子命令入口，通过 `--chunks` flag 区分模式。

#### Scenario: cdt session 默认返回复合视图

- **WHEN** 用户运行 `cdt session abc123`
- **THEN** SHALL 输出 summary + cost + errors 的合并视图
- **AND** table 格式 SHALL 紧凑展示核心指标

#### Scenario: cdt session --chunks 进入 chunk 模式

- **WHEN** 用户运行 `cdt session abc123 --chunks --tail 5 --content full`
- **THEN** SHALL 输出最后 5 条 chunk 的完整内容

#### Scenario: cdt sessions 支持全局查询

- **WHEN** 用户运行 `cdt sessions --since yesterday`（不带 --project）
- **THEN** SHALL 输出所有项目中昨天的 session 列表

#### Scenario: cdt sessions 支持 group-by

- **WHEN** 用户运行 `cdt sessions --since 7d --group-by project`
- **THEN** table 输出 SHALL 按项目分组显示

#### Scenario: cdt sessions 支持 branch 过滤

- **WHEN** 用户运行 `cdt sessions --branch feat/auth`
- **THEN** SHALL 只输出 gitBranch 含 "feat/auth" 的 session

#### Scenario: cdt session latest 解析

- **WHEN** 用户运行 `cdt session latest`
- **THEN** SHALL 解析为最近一次 session 并输出其复合视图

### Requirement: CLI 自动补全

`cdt completions <shell>` SHALL 生成包含新命令结构的 shell 补全脚本（bash/zsh/fish/powershell）。

自动补全 SHALL 覆盖：
- 顶级命令名（projects/sessions/session/search/stats/serve/mcp/setup/completions/self-update）
- `cdt session` 的位置参数 SHALL 提供 session ID 补全（基于最近 session 列表）
- `cdt sessions --project` 的参数值 SHALL 提供项目名补全
- `cdt sessions --since` 的参数值 SHALL 提供常用时间表达式补全（today/yesterday/7d/24h/30d）
- `cdt sessions --group-by` 的参数值 SHALL 提供枚举补全（none/project/day）
- `cdt session <id> --include` 的参数值 SHALL 提供 facet 枚举补全（phases/tools/activity/idle_gaps/files）
- `cdt session <id> --chunks --content` 的参数值 SHALL 提供模式补全（compact/overview/full）
- `cdt session <id> --chunks --filter` 的参数值 SHALL 提供枚举补全（errors_only/tool_calls）

#### Scenario: zsh 补全 session ID

- **GIVEN** 用户已 source 了 `cdt completions zsh` 的输出
- **WHEN** 用户输入 `cdt session ` 后按 Tab
- **THEN** SHALL 展示最近 session 的 ID 列表（通过 `SessionCompleter`）

#### Scenario: bash 补全 --since 值

- **GIVEN** 用户已 eval 了 `cdt completions bash` 的输出
- **WHEN** 用户输入 `cdt sessions --since ` 后按 Tab
- **THEN** SHALL 展示 today/yesterday/7d/24h/30d 等候选值

#### Scenario: zsh 补全 --include facets

- **WHEN** 用户输入 `cdt session abc --include ` 后按 Tab
- **THEN** SHALL 展示 phases/tools/activity/idle_gaps/files 候选值

### Requirement: 时间参数格式统一

CLI 的 `--since` 和 `--until` 参数 SHALL 与 MCP 的 `since`/`until` 接受完全相同的格式集：

- 相对时长：`7d`/`24h`/`1h`/`30m`
- 命名周期：`today`/`yesterday`/`week`
- 绝对日期：`2026-06-06`/ISO 8601

`--until` SHALL 作为新 flag 添加到 `cdt sessions` 命令。

#### Scenario: --since yesterday 与 MCP 行为一致

- **WHEN** 用户运行 `cdt sessions --since yesterday --format json`
- **THEN** 输出结果 SHALL 与 MCP `list_sessions({since: "yesterday"})` 返回的 items 一致

#### Scenario: --until 配合 --since 限定范围

- **WHEN** 用户运行 `cdt sessions --since 2026-06-01 --until 2026-06-03`
- **THEN** SHALL 只输出 [6月1日, 6月3日) 范围内的 session

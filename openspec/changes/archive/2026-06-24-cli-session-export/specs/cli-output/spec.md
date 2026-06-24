## MODIFIED Requirements

### Requirement: CLI 命令结构

CLI binary `cdt` SHALL 提供以下顶级命令结构：

- `cdt projects list` — 列出所有项目
- `cdt sessions list` — 列出 session（支持全局或按项目过滤）
- `cdt session <id>` — 单 session 复合视图（summary + cost + errors）
- `cdt session <id> --chunks` — chunk 级内容取数
- `cdt export <id>` — 导出会话为 Markdown / JSON 文档
- `cdt search <query>` — 全文搜索
- `cdt stats [period]` — 聚合统计
- `cdt serve` — HTTP API server
- `cdt mcp serve` — MCP stdio server
- `cdt setup` — 安装配置
- `cdt completions <shell>` — shell 补全脚本生成
- `cdt self-update` — 自更新

`cdt session <id>` 和 `cdt session <id> --chunks` 共用同一子命令入口，通过 `--chunks` flag 区分模式。

`session` 和 `export` 均支持 `latest` 作为 session ID 别名，解析为最近一次 session。

#### Scenario: cdt session 默认返回复合视图

- **WHEN** 用户运行 `cdt session abc123`
- **THEN** SHALL 输出 summary + cost + errors 的合并视图
- **AND** table 格式 SHALL 紧凑展示核心指标

#### Scenario: cdt session --chunks 进入 chunk 模式

- **WHEN** 用户运行 `cdt session abc123 --chunks --tail 5 --content full`
- **THEN** SHALL 输出最后 5 条 chunk 的完整内容

#### Scenario: cdt export 导出会话

- **WHEN** 用户运行 `cdt export <session-id>`
- **THEN** SHALL 以 Markdown 格式输出会话内容到 stdout
- **AND** 支持 `--export-format md/json`、`-o <path>`、`--detail`、`--no-thinking`、`--no-subagents`

#### Scenario: cdt export 与全局参数隔离

- **WHEN** 用户运行 `cdt export <id> --export-format md`
- **THEN** `--export-format` SHALL 为 export 子命令专用参数（md / json），与全局 `--format`（json / jsonl / table）隔离互不影响

#### Scenario: cdt sessions list 支持全局查询

- **WHEN** 用户运行 `cdt sessions list --since yesterday`（不带 --project）
- **THEN** SHALL 输出所有项目中昨天的 session 列表

#### Scenario: cdt sessions list 支持 group-by

- **WHEN** 用户运行 `cdt sessions list --since 7d --group-by project`
- **THEN** table 输出 SHALL 按项目分组显示

#### Scenario: cdt sessions list 支持 branch 过滤

- **WHEN** 用户运行 `cdt sessions list --branch feat/auth`
- **THEN** SHALL 只输出 gitBranch 含 "feat/auth" 的 session

#### Scenario: cdt session latest 解析

- **WHEN** 用户运行 `cdt session latest`
- **THEN** SHALL 解析为最近一次 session 并输出其复合视图

#### Scenario: cdt search --since 预过滤

- **WHEN** 用户运行 `cdt search "deploy" --since 7d`
- **THEN** SHALL 只搜索 7 天内的 session

#### Scenario: cdt stats --group-by 分组

- **WHEN** 用户运行 `cdt stats 7d --group-by model`
- **THEN** SHALL 按模型分组输出统计数据

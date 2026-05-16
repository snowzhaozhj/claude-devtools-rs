## Why

Rust 端当前虽然已有 `general.claudeRootPath` 配置字段，但项目发现、搜索、CLAUDE.md/auto-memory 读取和 watcher 仍实际绑定默认 `~/.claude`，无法使用用户指定的 Claude 数据根目录。原版支持自定义 Claude root，并在未配置时自动回退默认目录；Rust 端需要补齐这一行为，避免多 Claude 数据目录、迁移目录或隔离测试环境下无法发现项目。

## What Changes

- `general.claudeRootPath` 作为 Claude 数据根目录配置：为空时使用默认 home 下 `.claude`，非空时必须是绝对路径。
- 项目发现、session 搜索、CLAUDE.md/auto-memory 读取、file watcher 的 `projects`/`todos` 路径 SHALL 基于当前 Claude root 计算。
- Settings UI 提供 Claude root 配置入口，支持通过系统目录选择器选择目录、手动填写绝对路径、清空恢复默认，并持久化到配置文件。
- 运行中更新 `general.claudeRootPath` 后，项目发现、session 搜索、CLAUDE.md/auto-memory 读取 SHALL 立即使用新 root；file watcher 在下次启动时使用新 root。

## Capabilities

### New Capabilities

- 无。

### Modified Capabilities

- `configuration-management`: 明确 `general.claudeRootPath` 的默认值、绝对路径校验和持久化语义。
- `project-discovery`: 项目发现根目录从固定默认改为当前 Claude root 下的 `projects`。
- `session-search`: session 搜索根目录从固定默认改为当前 Claude root 下的 `projects`。
- `file-watching`: watcher 监听根目录从固定默认改为当前 Claude root 下的 `projects`/`todos`。
- `settings-ui`: 增加 Claude root 配置入口与恢复默认交互。

## Impact

- Rust crates: `cdt-config`、`cdt-discover`、`cdt-watch`、`cdt-api`。
- Tauri backend: 启动时需要把当前 Claude root 传入 scanner/searcher/watcher/API 上下文；配置更新时需要重配 scanner/searcher/API 上下文。
- UI: `GeneralConfig` 类型、Settings 页面表单和 mock fixture 需要同步。
- Tests: 配置 round-trip、project discovery/search custom root、watcher path、Settings UI 单测或 contract fixture 需要覆盖。

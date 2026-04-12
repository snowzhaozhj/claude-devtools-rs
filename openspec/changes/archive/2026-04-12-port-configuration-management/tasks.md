## 1. 依赖 + 脚手架

- [x] 1.1 在 workspace `Cargo.toml` 的 `[workspace.dependencies]` 中添加 `dirs`（home dir）依赖声明；为 `cdt-config` 的 `[dependencies]` 添加 `serde`、`serde_json`、`tokio`（fs feature）、`regex`、`dirs`、`tracing`、`thiserror`、`cdt-core`
- [x] 1.2 建立 `cdt-config/src/` module 结构：`lib.rs`、`types.rs`、`defaults.rs`、`error.rs`、`manager.rs`、`trigger.rs`、`claude_md.rs`、`mention.rs`、`validation.rs`、`regex_safety.rs`
- [x] 1.3 `cargo build -p cdt-config` 确认编译通过

## 2. 类型定义 + 默认值

- [x] 2.1 在 `types.rs` 定义 `AppConfig`、`NotificationConfig`、`GeneralConfig`、`DisplayConfig`、`SessionsConfig`、`SshPersistConfig`、`HttpServerConfig`、`NotificationTrigger`、`TriggerMode`、`TriggerContentType` 等，derive `Serialize` / `Deserialize` / `Clone` / `Debug`
- [x] 2.2 在 `defaults.rs` 定义 `DEFAULT_CONFIG` 和 `DEFAULT_TRIGGERS`
- [x] 2.3 在 `error.rs` 定义 `ConfigError` thiserror enum（`Io`、`Json`、`Validation`、`PathEscape`）

## 3. Regex 安全校验

- [x] 3.1 在 `regex_safety.rs` 实现 `validate_regex_pattern`：长度限制（100）、危险 pattern 检测、括号平衡、`regex::Regex::new` 语法验证
- [x] 3.2 实现 `create_safe_regex` 便利函数
- [x] 3.3 单元测试：合法 pattern、过长 pattern、嵌套量词、不平衡括号、无效语法

## 4. 配置字段校验

- [x] 4.1 在 `validation.rs` 实现 `validate_config_update` 分 section 校验（notifications / general / display / httpServer / ssh）
- [x] 4.2 实现 `normalize_claude_root_path`：空/非绝对路径 → `None`，去尾斜杠
- [x] 4.3 单元测试：无效端口（0、70000）、有效端口（3456）、非法 section、空 claudeRootPath

## 5. Trigger 管理

- [x] 5.1 在 `trigger.rs` 实现 `TriggerManager`：`add`、`update`、`remove`（builtin 不可删）、`get_all`、`get_enabled`
- [x] 5.2 实现 `validate_trigger`：必填字段、mode 特异校验（`content_match` 需 matchField、`token_threshold` 需 threshold ≥ 0）、ignore pattern ReDoS 校验
- [x] 5.3 实现 `merge_triggers`：保留用户已有 trigger + 补齐缺失 builtin + 移除过期 builtin
- [x] 5.4 单元测试：添加/更新/删除、builtin 不可删、merge 补齐、校验失败

## 6. ConfigManager 核心

- [x] 6.1 在 `manager.rs` 实现 `ConfigManager`：`new`（接受 path + `FileSystemProvider`）、`load`（async）、`save`
- [x] 6.2 实现损坏文件备份逻辑：parse 失败 → `rename` 到 `.bak.<timestamp>` → 加载默认 → 持久化（修复 TS impl-bug）
- [x] 6.3 实现 `merge_with_defaults`：partial JSON 与默认值递归合并
- [x] 6.4 实现 `update_config`：分 section 更新 + 校验 + persist
- [x] 6.5 实现 session pin/unpin、hide/unhide、snooze 管理
- [x] 6.6 单元测试：首次启动无文件、损坏文件备份+恢复、partial merge、update 校验拒绝、pin/unpin

## 7. CLAUDE.md 读取

- [x] 7.1 在 `claude_md.rs` 实现 `read_claude_md_file`：单文件读取 → `ClaudeMdFileInfo`（path / exists / char_count / estimated_tokens）
- [x] 7.2 实现 `read_directory_md_files`：递归收集 `*.md` → 合并统计
- [x] 7.3 实现 `read_all_claude_md_files`：8 scope 枚举读取，返回 `BTreeMap<Scope, ClaudeMdFileInfo>`
- [x] 7.4 实现 `read_auto_memory_file`：只读前 200 行
- [x] 7.5 单元测试：只有 user scope、全 8 scope、文件不存在、permission denied mock、auto-memory 截断

## 8. @mention 路径沙盒

- [x] 8.1 在 `mention.rs` 定义 `SENSITIVE_PATTERNS`（`LazyLock<RegexSet>`）和 `validate_file_path` 函数
- [x] 8.2 实现允许目录检查：project root + `~/.claude/`
- [x] 8.3 实现 symlink escape 防护：`fs::canonicalize` 后再检白名单
- [x] 8.4 实现 `read_mentioned_file`：校验 → 读取 → token 限制检查
- [x] 8.5 单元测试：合法路径、traversal 攻击、敏感文件拦截、symlink escape、token 超限

## 9. lib.rs 公开 API + 集成

- [x] 9.1 在 `lib.rs` 通过 `pub use` 导出公开 API：`ConfigManager`、`ClaudeMdFileInfo`、`AppConfig`、`validate_file_path`、`read_all_claude_md_files` 等
- [x] 9.2 `cargo clippy --workspace --all-targets -- -D warnings` 全量检查
- [x] 9.3 `cargo fmt --all`
- [x] 9.4 `cargo test -p cdt-config` 全测试通过

## 10. 文档 + 收尾

- [x] 10.1 更新根 `CLAUDE.md` 的 Capability→crate map：`configuration-management` → `done ✓`
- [x] 10.2 更新 `CLAUDE.md` 的 "Known TS impl-bugs" 段：标记 configuration-management impl-bug 为 ✓
- [x] 10.3 更新 `openspec/followups.md`：标记 configuration-management 条目为已修
- [x] 10.4 `openspec validate port-configuration-management --strict` 通过

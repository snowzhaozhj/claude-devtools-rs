## Context

`cdt-config` 当前是 stub（空 `config` / `triggers` 两个 module）。TS 侧实现分布在 `ConfigManager.ts`（964 行）、`TriggerManager.ts`（317 行）、`ClaudeMdReader.ts`（318 行）、`configValidation.ts`（461 行）、`regexValidation.ts`（183 行）、`pathValidation.ts`（267 行）。

TS 的 `ConfigManager` 是 singleton + 异步 `initialize()`；Rust 不需要 singleton pattern，但需要异步 load/save。

`cdt-analyze::context` 已预留 `initial_claude_md_injections` 注入点，等待本 port 提供 CLAUDE.md 读取结果。

## Goals / Non-Goals

**Goals:**
- 完整实现 spec 定义的 5 个 Requirement（persist config、read/update、read CLAUDE.md、resolve mentions、validate fields）
- 修复 impl-bug：损坏 config 自动备份
- CLAUDE.md 读取覆盖 TS 实际的 8 scope（spec 仅定义 3 scope，MODIFIED delta 对齐）
- Trigger 数据类型 + CRUD + 持久化（不含 eval 逻辑）
- `@mention` 路径沙盒（敏感文件黑名单 + 允许目录白名单 + symlink 防护）
- 配置字段校验（port 范围、regex ReDoS 防护）

**Non-Goals:**
- Trigger 评估/匹配逻辑 → 留给 `port-notification-triggers`
- SSH profile 的连接管理 → 留给 `port-ssh-remote-context`
- IPC / HTTP handler 层 → 留给 `port-ipc-data-api` / `port-http-data-api`
- UI 偏好的实际消费（theme、compact mode 等）→ UI 层
- `cdt-analyze::context` 的 `initial_claude_md_injections` 接入 → 单独 PR 或同 change 尾部 task

## Decisions

### D1: Module 结构

```
cdt-config/src/
├── lib.rs              # pub mod + re-export
├── types.rs            # AppConfig / NotificationConfig / GeneralConfig 等 serde 类型
├── defaults.rs         # DEFAULT_CONFIG + DEFAULT_TRIGGERS
├── manager.rs          # ConfigManager: load / save / update / merge
├── trigger.rs          # TriggerManager: CRUD + validate + merge
├── claude_md.rs        # ClaudeMdReader: 8 scope 读取
├── mention.rs          # @mention 路径解析 + 沙盒
├── validation.rs       # 配置字段校验（section 级）
├── regex_safety.rs     # ReDoS 防护 regex 校验
└── error.rs            # ConfigError thiserror enum
```

**理由**：每个 TS 源文件大致对应一个 Rust module，保持 1:1 可追溯性。`types.rs` 独立放置，因为 `cdt-api` 层也会引用这些类型。

### D2: 配置文件路径与 `FileSystemProvider` 复用

复用 `cdt-core::FileSystemProvider` trait 做 fs 抽象（已在 `port-project-discovery` 落地）。`ConfigManager` 接受 `&dyn FileSystemProvider` 参数，测试时注入 mock。

配置路径默认 `~/.claude/claude-devtools-config.json`，与 TS 一致。

### D3: 损坏文件备份策略

当 JSON parse 失败时：
1. 用 `fs::rename` 将损坏文件移动到 `<path>.bak.<unix_timestamp_ms>`
2. `tracing::warn!` 记录备份路径
3. 加载 DEFAULT_CONFIG 并持久化

**替代方案**：copy + delete（两步操作，crash-unsafe）。选择 rename（原子操作）。

### D4: Regex ReDoS 防护

移植 TS 的 `regexValidation.ts` 逻辑：
- 最大长度 100 字符
- 检测危险 pattern（嵌套量词、重叠替代等）
- 括号平衡检查
- `regex::Regex::new()` 语法验证

Rust 的 `regex` crate 本身有 O(n) 保证（不使用 backtracking），但仍保留 pattern 校验以与 TS 行为一致、拦截明显误写。

### D5: CLAUDE.md 8 scope 读取

Spec 只定义 3 scope（global / project / directory），但 TS 实际实现 8 scope。MODIFIED delta 将 spec 对齐到 8 scope。

| Scope | 路径 |
|---|---|
| enterprise | 平台相关（macOS: `/Library/Application Support/ClaudeCode/CLAUDE.md`）|
| user | `~/.claude/CLAUDE.md` |
| project | `<project>/CLAUDE.md` |
| project-alt | `<project>/.claude/CLAUDE.md` |
| project-rules | `<project>/.claude/rules/**/*.md` |
| project-local | `<project>/CLAUDE.local.md` |
| user-rules | `~/.claude/rules/**/*.md` |
| auto-memory | `~/.claude/projects/<encoded>/memory/MEMORY.md`（前 200 行）|

Token 估算：`content.len() / 4`（与 TS `countTokens` 一致，简单整除）。

### D6: `@mention` 沙盒

- 敏感文件 pattern 黑名单（~20 条 regex）→ 编译为 `regex::RegexSet`，`LazyLock` 初始化一次
- 允许目录白名单：project root + `~/.claude/`
- Symlink escape 防护：`fs::canonicalize` 后再检查一次白名单

### D7: 异步策略

`ConfigManager` 的 `load` / `save` / CLAUDE.md 读取都是 async（tokio fs）。`types.rs` / `defaults.rs` / `validation.rs` / `regex_safety.rs` 是纯同步。

## Risks / Trade-offs

- **[Risk] 配置 schema 演进** → 用 `mergeWithDefaults` 策略（新字段自动填默认值），与 TS 一致。新增字段只需改 `defaults.rs`。
- **[Risk] `regex` crate 与 JS RegExp 语义差异** → 对于用户输入的 trigger pattern，部分 JS regex 语法（如 lookahead）在 Rust `regex` 中不支持。记录在 MODIFIED delta 中，不做兼容层。
- **[Trade-off] Token 估算精度** → `len / 4` 是粗略估算，TS 用同样的方法。精确 tokenizer 增加依赖且对本应用价值低。

# bug-hunt 反模式 checklist + grep recipe

> 各 lens 的具体反模式清单。SKILL.md::Step 2 引用本文。
> 体例：每条反模式给 **抓手 grep / 特征**、**典型例子**、**常见误报**（哪些看着像但其实不是）。

## 目录

- [L1 silent failures](#l1-silent-failures-吞错--默默 fallback)
- [L2 边界 + 状态机](#l2-边界--状态机)
- [L3 并发 + 资源](#l3-并发--资源)
- [L4 跨域契约](#l4-跨域契约)
- [L5 安全](#l5-安全)
- [L6 测试伪覆盖](#l6-测试伪覆盖)

---

## L1 silent failures（吞错 / 默默 fallback）

社区共识：silent failure 是远比 panic 更危险的 bug——panic 至少有 stack trace，silent failure 让数据慢慢偏离正确状态。

### L1.1 `.unwrap()` / `.expect()` 在用户输入路径

**抓手**：
```bash
rg '\.unwrap\(\)|\.expect\(' --type rust -n <scope>
```

**真 bug 信号**：
- 在公共 IPC handler / `#[tauri::command]` / HTTP route handler 里
- 操作的对象是用户传入的 `String` / `PathBuf` / `Vec<u8>` / 网络数据
- 失败会 panic 整个 runtime（而不是单个请求）

**误报信号**（**不是** bug）：
- 在测试 `#[cfg(test)]` 块里
- 在 `Lazy::new` / `OnceCell::new` 这类初始化常量
- 操作字面量 / 编译期已知值（`"abc".parse::<i32>().unwrap()`）
- 已经在上层做了校验，下层 unwrap 是约定（应配 comment 说明）

### L1.2 `let _ = result;` 弃 `Result`

**抓手**：
```bash
rg 'let _ = .*\?\s*$|let _ = .*\.send\(|let _ = .*\.write\(|let _ = .*fs::' --type rust -n
```

**真 bug 信号**：弃掉的 Result 含 I/O 失败 / 写入失败 / channel send 失败 — 错误被吞，用户感知不到操作没成功。

**误报信号**：
- `let _ = tx.send(...)` 在已知 receiver 可能 dropped 的清理路径（如 graceful shutdown）
- 单元测试里 `let _ = setup_thing()` 故意忽略

### L1.3 `.ok()` 把 `Result` 转 `Option` 后吞 None

**抓手**：
```bash
rg '\.ok\(\)' --type rust -n
```

**真 bug 信号**：链式 `something.parse().ok().unwrap_or_default()` —— parse 失败默默回零值，用户以为成功。

**误报信号**：明确预期 None 的兜底场景（如 "可选配置项"）。

### L1.4 catch + log + continue 不抛

**抓手**：
```bash
rg 'Err\(.*\) =>.*log|Err\(.*\) =>.*tracing|Err\(_\) =>' --type rust -n
```

**真 bug 信号**：错误只 log 不传播，循环里继续跑，最终用户看到的是"成功"但部分数据丢失。

### L1.5 默默 fallback 到默认值掩盖错误

**抓手**：
```bash
rg '\.unwrap_or\(|\.unwrap_or_default\(|\.unwrap_or_else\(' --type rust -n
```

**真 bug 信号**：fallback 值是 `0` / 空集合 / 空字符串，且 caller 无法区分"成功且为 0"和"失败 fallback 为 0"。

**误报信号**：fallback 是合理 default（如 配置项缺失用 default config）+ 上下文有日志。

---

## L2 边界 + 状态机

### L2.1 `as` 强转截断（u64 → u32 / usize → i32 / etc）

**抓手**：
```bash
rg ' as u(8|16|32) | as i(8|16|32) | as usize\s*[+\-]' --type rust -n
```

**真 bug 信号**：源类型范围 > 目标类型，且源值可能由用户输入控制（如 文件 size / 数组长度 / time delta）。

**修法对照**：用 `try_from` / `try_into` 显式处理截断。

### L2.2 `len() - 1` 不查空

**抓手**：
```bash
rg '\.len\(\)\s*-\s*1' --type rust -n
```

**真 bug 信号**：`vec.len() - 1` 在 `vec` 可能为空时 → underflow panic（`usize` 减法）。

**修法对照**：`vec.last()` / `vec.split_last()`。

### L2.3 索引访问不查界 `[0]` / `[i]`

**抓手**：
```bash
rg '\[0\]|\[\d+\]' --type rust -n
```

**真 bug 信号**：操作可能空集合 + 索引来自用户输入或循环不变量未保证。

**误报**：iter 后立刻 `.next().unwrap()` 紧接判 `is_empty()` 已确保非空。

### L2.4 `match` 用 `_ =>` 兜底吞新 enum 分支

**抓手**：
```bash
rg '_ => ' --type rust -n -A 2
```

**真 bug 信号**：被 match 的 enum 是 `#[non_exhaustive]` 或近期加过新 variant，`_` arm 静默吞掉。

**修法对照**：显式列每个 variant，加 enum variant 时 compiler 强制处理。

### L2.5 状态机漏转移

**抓手**：找 `enum State { ... }` + `fn transition` 看每个 state × event 的组合是否都有处理。

**真 bug 信号**：状态机在 `(StateA, EventX)` 组合下静默 drop event 不做任何处理 / 不报错。

### L2.6 TOCTOU（Time-of-check vs time-of-use）

**抓手**：
```bash
# 先 exists 后 open / 先查 metadata 后操作
rg '\.exists\(\)' --type rust -n -A 5 | rg '(File::open|fs::read|fs::write|metadata)'
```

**真 bug 信号**：`if path.exists() { File::open(path) }` —— 中间 path 可能被删，Open 失败。直接 `File::open` 看错误处理即可。

---

## L3 并发 + 资源

### L3.1 锁顺序不一致 / 嵌套锁

**抓手**：找同一 struct 持有 ≥ 2 个 `Mutex` / `RwLock`，看不同函数的锁取用顺序。
```bash
rg 'Arc<Mutex<|Arc<RwLock<' --type rust -n
```

**真 bug 信号**：函数 A 先锁 X 再锁 Y，函数 B 先锁 Y 再锁 X → 死锁。

### L3.2 `.lock().await` 持有跨 await 点

**抓手**：
```bash
rg '\.lock\(\)\.await' --type rust -n -A 10
```

**真 bug 信号**：拿了 tokio Mutex 后中间 `.await` 别的 future（特别是 I/O / channel send）→ 长时间持锁阻塞其他 task。

**修法对照**：取出数据 → drop guard → 再 await。

### L3.3 `broadcast::channel(N)` capacity 过小

**抓手**：
```bash
rg 'broadcast::channel\(' --type rust -n
```

**真 bug 信号**：capacity < 32 + 慢 subscriber + lagging 触发 `RecvError::Lagged` 丢消息（用户感知不到）。

**修法对照**：`broadcast::channel(128)` 起步 + lag 处理走重新订阅 / 重 sync 路径。

### L3.4 `tokio::spawn` 无 cancel 句柄 + 长生命周期

**抓手**：
```bash
rg 'tokio::spawn' --type rust -n
```

**真 bug 信号**：spawn 出去的 task 持有大对象 / channel sender / file handle，task 永不退出 → 资源泄漏 / shutdown 时挂起。

**修法对照**：用 `JoinHandle` 显式 abort / 配 `CancellationToken` / 跑完即退。

### L3.5 `std::fs::*` / `std::process::Command::output()` 在 async 函数

**抓手**：
```bash
rg 'async fn ' --type rust -A 30 | rg '(std::fs::|Command::new.*output\(\))'
```

**真 bug 信号**：阻塞 tokio worker thread → 整个 runtime 卡。

**修法对照**：`tokio::fs` / `tokio::process::Command` / `tokio::task::spawn_blocking`。

### L3.6 unbounded cache（无 byte / count cap）

**抓手**：
```bash
rg 'HashMap<|BTreeMap<|Vec<' --type rust -n -A 5 | rg '(insert|push)'
```

**真 bug 信号**：长生命周期的 Map / Vec 只 insert 不 evict，N 增长无上限 → OOM。

**修法对照**：LRU + TTL + double cap (count + bytes)。本仓 perf rules 已硬约束（`.claude/rules/perf.md::反模式清单 内存类`）。

### L3.7 `subscribe()` 后无显式 drop / unsubscribe

**抓手**：找所有 `.subscribe()` / `add_listener` / `register_callback` 用法看是否有 unregister 路径。

**真 bug 信号**：subscriber 用完不退订 → 长生命周期里堆积 → broadcast lag。

---

## L4 跨域契约

### L4.1 IPC 字段名/类型 ui ↔ src-tauri 不对齐

**抓手**：
```bash
# 找 #[tauri::command] 函数
rg '#\[tauri::command\]' --type rust -n -A 3
# 比对 ui/src 的 invoke 调用
rg 'invoke\(' ui/src/ -n
```

**真 bug 信号**：rust 端字段叫 `session_id`（snake_case 出去 → camelCase `sessionId`），但前端 invoke 写 `session_id` → IPC 反序列化失败。本仓硬约束在 `crates/CLAUDE.md::serde 命名` + `src-tauri/CLAUDE.md::IPC 字段改动 checklist`。

### L4.2 `#[serde(rename_all = "camelCase")]` 漏写 / 写错

**抓手**：
```bash
rg '#\[serde\(' --type rust -n
```

**真 bug 信号**：导出给 ui 的 struct 缺 `#[serde(rename_all = "camelCase")]`，前端访问 `obj.fooBar` 拿到 undefined。

### L4.3 跨 crate 公共 API breaking change 无 migration

**抓手**：
```bash
rg 'pub fn |pub struct |pub enum ' --type rust -n
```

**真 bug 信号**：改了 `cdt-core` 的公共 fn 签名，但 `cdt-api` / `cdt-cli` 没同步改 → 编译过但行为变。

### L4.4 Windows 兼容反模式

**抓手**：
```bash
rg 'Path::is_absolute|dirs::home_dir|/[a-z]' --type rust -n
```

**真 bug 信号**（本仓踩过的坑，详 `windows-compat-reviewer` agent）：
- `Path::is_absolute()` 在 Windows 下 `C:\foo` 返回 true 但 `\foo` 也 true → 误判
- 硬编码 `/` 路径分隔符
- 私有 `encode_path` 副本（应统一走 cdt-core 的）
- 测试里 `encode_path(windows_path)` 当真磁盘目录名

### L4.5 `cfg(target_os = "X")` 漏分支

**抓手**：
```bash
rg '#\[cfg\(target_os' --type rust -n
```

**真 bug 信号**：`#[cfg(target_os = "macos")]` + `#[cfg(target_os = "linux")]` 没有 `#[cfg(target_os = "windows")]` → Windows 构建编译错或缺功能。

---

## L5 安全

### L5.1 `Command::new` 拼接用户输入

**抓手**：
```bash
rg 'Command::new\(' --type rust -n -A 5 | rg 'arg\(.*format!|arg\(&format!|arg\(.*\+'
```

**真 bug 信号**：用户输入直接拼进 shell 命令 → 命令注入。本仓有 SSH 路径需特别关注。

**修法对照**：`Command::args(&[...])` 传数组，禁用 shell 解析。

### L5.2 路径遍历 `..`

**抓手**：
```bash
rg 'PathBuf::from|Path::new' --type rust -n -A 3 | rg '(format!|push)' 
```

**真 bug 信号**：拼路径前没 `canonicalize` / 没拒 `..` segments → 用户可逃出 sandbox。

### L5.3 反序列化外部数据无 size 限制

**抓手**：
```bash
rg 'serde_json::from_str|serde_json::from_slice|bincode::deserialize' --type rust -n
```

**真 bug 信号**：从 SSH / HTTP / IPC 收到的数据无 size cap → DoS。

### L5.4 `format!` 拼 SQL / path / cmd

**抓手**：
```bash
rg 'format!\(.*\{.*\}.*\)' --type rust -n | rg '(query|exec|path|cmd|url)'
```

**真 bug 信号**：用户输入经 `format!` 拼到敏感字符串 → 注入。

---

## L6 测试伪覆盖

### L6.1 mock 替代真实路径

**抓手**：
```bash
rg 'MockX|mockall|#\[mockable\]|fn mock_' --type rust -n
```

**真 bug 信号**：测试用 mock 替代了**正是要测的那个组件**（如 IPC handler 测试 mock 掉了 IPC layer）→ 测试 pass 但真路径没跑。本仓已有 `e2e-http-verify` skill 专治这坑。

**修法对照**：mock 用在外部依赖（数据库 / 远程服务），不要 mock 自己写的核心逻辑。

### L6.2 scenario 名对得上但 assert 不验关键字段

**抓手**：openspec scenario 的关键 SHALL 句逐句对应 test 函数的 assert。
```bash
# 找 spec scenario
rg 'SHALL' openspec/specs/<cap>/spec.md
# 看 test 是否真验那个字段
rg 'assert' <test_file>
```

**真 bug 信号**：spec 说"SHALL 把 X 字段返回为 Y"，test 名叫 `test_x_returns_y` 但只 assert 了"调用没 panic"。

### L6.3 `assert!(true)` / 空 test 占位

**抓手**：
```bash
rg 'assert!\(true\)|fn test_.*\{\s*\}' --type rust -n
```

**真 bug 信号**：占位 test 凑数，CI 计入 pass 但实际没测什么。

### L6.4 只测 happy path 不测 error / empty / N=1

**抓手**：grep test 函数名找 `_empty` / `_error` / `_single` / `_boundary` 后缀；少则疑。

**真 bug 信号**：业务函数 5 条分支，test 只覆盖 1 条主线 → 4 条隐藏路径未保护。

### L6.5 ignored test 长期不修

**抓手**：
```bash
rg '#\[ignore\]' --type rust -n
```

**真 bug 信号**：`#[ignore]` 注释里写"TODO 暂时挂"半年没动 → 该路径已无任何测试保护。

---

## 跨 lens 通用：本仓已踩过的坑（高优先级抓手）

社区共识 + 本仓 incident 沉淀：

| 反模式 | 沉淀文档 | 本 skill lens |
|---|---|---|
| `tokio::runtime::Runtime::new()` 默认配 + Tauri 同进程并存 | `.claude/rules/perf.md::反模式清单 runtime/调度类` | L3 |
| `tracing_subscriber` layer 未 `.with_filter` 包 | 同上 | L3 |
| Windows `Path::is_absolute()` / 硬编码 `/` | `windows-compat-reviewer` agent | L4 |
| IPC payload > 1 MB 不瘦身 | `src-tauri/CLAUDE.md::IPC payload 瘦身模式` | L4 / L3 |
| `content-visibility: auto + contain-intrinsic-size` 估算高度 | `.claude/rules/perf.md::反模式清单 前端渲染类` | L2 / L3 (滚动状态)|
| 同步循环 `serde_json::from_str` 大 JSON | `.claude/rules/perf.md` | L3 |
| broadcast subscriber 链式放大 5+ | 同上 | L3 |
| `cfg(target_os)` 漏 windows 分支 | 本仓 cross-platform CI gate | L4 |

按 scope 优先抓上面这些——历史已验证为真 bug 的反模式比理论反模式更值得投入精力查。

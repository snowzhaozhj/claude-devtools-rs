# bug-hunt 误报防御场景

> "看着像 bug 但其实不是"。SKILL.md::真实性闸门 引用本文。
> 写每条 finding 前过一遍这里——命中即降置信度或丢。

## 总原则

LLM 找 bug 的误报源 80% 来自**只看局部不看上下文**。一个反模式片段单看必中，但放回完整调用链 / 测试覆盖 / 项目约定后就不是 bug。

**核心防御 4 题**：每条 candidate 用这 4 题自校：

1. **上下文已校验过吗**：调用方 / 上层是否已经做过 null check / range check / type check？
2. **是测试代码 / 工具代码吗**：`#[cfg(test)]` / `tests/` / `examples/` / `benches/` 里的反模式权重低
3. **是初始化常量吗**：`static` / `const` / `Lazy::new` / `OnceCell` 里的 `unwrap` 通常是约定不是 bug
4. **项目有显式约定吗**：CLAUDE.md / 文档 / 注释明说"这里约定可以 unwrap" → 不是 bug

---

## L1 silent failures 误报场景

### `unwrap()` 不一定是 bug

**不是 bug 的 unwrap 场景**：

- **测试代码**：`#[cfg(test)] mod tests` 里 `unwrap()` 是惯例，失败让测试 fail 比写 `Result` 处理简洁
- **初始化常量**：`Regex::new(r"..").unwrap()` 在 module 顶层 / `Lazy::new` 里，编译期 regex 字符串保证不会失败
- **字面量 parse**：`"42".parse::<i32>().unwrap()` 操作字面量
- **已上层校验**：`fn inner(x: NonZero) { x.get().checked_div(...).unwrap() }` —— 类型已保证非零
- **示例 / bench 代码**：`examples/` / `benches/` 里 panic 是可接受的——非生产路径

**报为 bug 的判别**：unwrap 在**用户输入路径**（IPC / HTTP / file system / SSH）+ 失败会 panic 整个进程。

### `let _ = ...` 不一定是吞错

**不是 bug 的弃 Result 场景**：

- **graceful shutdown 清理**：`let _ = tx.send(...)` 在 receiver 已 drop 的清理路径，预期失败
- **best-effort 通知**：`let _ = notify_user(...)` 通知失败不影响主功能（应配 comment 或 log）
- **故意丢弃返回值**：`let _: u32 = expr;` 类型标注用法，不是弃 Result

**报为 bug 的判别**：弃的 Result 含 I/O 失败 / 状态变更失败，且 caller 行为依赖该操作真成功。

### `unwrap_or_default()` 不一定是掩盖错误

**不是 bug 的 fallback 场景**：

- **配置项缺失用 default**：`config.timeout.unwrap_or(Duration::from_secs(30))` 是合理 default
- **可选字段**：解析 JSON 时 `obj.get("optional_field").unwrap_or_default()`

**报为 bug 的判别**：fallback 值与"真实 0"无法区分 + caller 用 fallback 值做了重要判断（数据丢失 / silent corruption）。

---

## L2 边界 + 状态机 误报场景

### `len() - 1` 不一定 underflow

**不是 bug**：上文已 `if !vec.is_empty()` / `if vec.len() > 0` 守卫过。

**报为 bug 的判别**：往上 5 行没找到非空检查 + vec 来源是用户输入 / 函数参数。

### `as` 强转不一定截断

**不是 bug**：源类型范围 ≤ 目标类型（`u8 as u32` / `i32 as i64`），或 caller 文档说 "i32 always fits in u32"。

**报为 bug 的判别**：源类型范围 > 目标类型 + 源值由用户控制（如 文件 size / 网络包长度）。

### `_ =>` 兜底不一定吞 enum 新分支

**不是 bug**：被 match 的 enum 是稳定的（无 `#[non_exhaustive]` + 近期无新 variant + 此处 `_` 行为是合理 default）。

**报为 bug 的判别**：enum 是 `#[non_exhaustive]` / 近期 git log 加过新 variant + `_` arm 静默 drop event。

---

## L3 并发 + 资源 误报场景

### `Arc<Mutex<>>` 不一定有死锁风险

**不是 bug**：进程内只有 1 个 `Mutex` / 多个 `Mutex` 但每个函数只取 1 个 / 锁顺序严格统一。

**报为 bug 的判别**：≥ 2 个 Mutex 嵌套使用 + 不同函数的取用顺序不一致。

### `tokio::spawn` 不一定泄漏

**不是 bug**：spawn 出去的 task 是 self-contained 短任务（如 一次性发邮件），自然结束。

**报为 bug 的判别**：task 持有长生命周期资源（channel sender / file handle / 大对象）+ 无显式终止条件 → shutdown 时挂起。

### `std::fs::*` 在 async 不一定阻塞

**不是 bug**：操作的是非常小的文件（< 1 KB）+ 路径已知绝对路径（无 syscall stat 链）。

**报为 bug 的判别**：操作的是用户路径 / 大文件 / 网络挂载点（macOS Spotlight / NFS） → 可能阻塞数百 ms。

### `broadcast::channel(N)` 小 capacity 不一定丢消息

**不是 bug**：subscriber 处理速度恒定快于 publisher + capacity 经测算够吸收 burst。

**报为 bug 的判别**：subscriber 涉及 I/O / 慢逻辑 + capacity < 32 + 无 lag 处理路径。

---

## L4 跨域契约 误报场景

### 字段名不对齐不一定是 bug

**不是 bug**：rust 端 `pub struct` 是内部 type 不出 IPC，前端从未访问该字段。

**报为 bug 的判别**：该 struct 经 `#[tauri::command]` 返回 / 经 IPC 传输 + 前端代码里有访问该字段。

### `cfg(target_os)` 漏分支不一定影响构建

**不是 bug**：项目已声明只支持 macOS / Linux（`Cargo.toml` 或 README 写明）。

**报为 bug 的判别**：本仓 CI 矩阵跑 windows-latest（`grep windows .github/workflows/*.yml`） + 该代码在 windows 矩阵 build 失败或行为不正确。

### `Path::is_absolute()` 不一定有 windows 陷阱

**不是 bug**：路径来源已规范化 / 已经经过 `canonicalize` / 测试只跑非 windows 平台。

**报为 bug 的判别**：路径来自用户输入 + 后续逻辑依赖 "absolute = 完整磁盘路径" 假设 + windows 矩阵在 CI 跑。

---

## L5 安全 误报场景

### `Command::new` 拼接不一定是注入

**不是 bug**：拼接的是**已知白名单**（如 `git` / `ls` 等固定命令）+ 参数走 `args(&[...])` 数组形式（不经 shell）。

**报为 bug 的判别**：拼接含用户输入字符串 + 经 shell 解析（`bash -c`） / 经 `format!` 拼整条命令字符串。

### `format!` 拼路径不一定是路径遍历

**不是 bug**：拼接的是已 canonicalize 后的路径 + 后续无 `fs::read` / `fs::write`。

**报为 bug 的判别**：拼接包含用户输入 + 后续有 `fs::*` 操作 + 无 `..` 拒绝逻辑。

### 反序列化无 size 限制不一定是 DoS

**不是 bug**：数据来源是**本进程内信任源**（自己写文件自己读 / 进程内 channel）+ size 已被上游 cap 过。

**报为 bug 的判别**：数据来自外部（HTTP / SSH / IPC from untrusted source） + 无 size cap。

---

## L6 测试伪覆盖 误报场景

### mock 测试不一定是伪覆盖

**不是 bug**：mock 替代的是**外部依赖**（数据库 / 远程服务 / 时间 / 随机数）+ 业务逻辑本身被真实跑。

**报为 bug 的判别**：mock 替代了**正是要测的那个组件**（mock IPC 测 IPC handler / mock parser 测 parser）。

### 只测 happy path 不一定是 bug

**不是 bug**：业务函数本身只有 happy path（如 纯函数 `fn add(a, b) { a + b }`），无 error 分支可测。

**报为 bug 的判别**：函数有显式 `Result<_, _>` 返回 / 多分支 `match` / 多条件分支 + test 只覆盖 1 条。

### `#[ignore]` 不一定是放弃覆盖

**不是 bug**：标 `#[ignore]` 是因为该 test 跑慢需 `--ignored` 显式触发（如 性能 bench / 集成测试），CI 有专门 job 跑。

**报为 bug 的判别**：`#[ignore]` 注释写"暂时挂"/"TODO" + 无任何 CI job 跑这部分 + 已超 2 周没改。

---

## 元规则：当我不确定是真 bug 时

**默认丢，不上报**。社区经验：误报一次伤公信力 ≈ 错过真 bug 三次。宁可漏报让用户后续发现（再 hunt 一遍），也不要把不确定的当真 bug 报上去让用户失望。

**留痕但不上报**：实在觉得有可疑但无法 100% 证实的 → 进 SKILL.md::Step 5 报告的"开放问号"段落，标 `< 50% 置信`，让用户决定要不要深 dig。

**给自己留台阶**：真 bug 找到 1 条 confirmed/critical 比报 10 条 medium/major 更有价值——用户会用前者，会忽略后者。

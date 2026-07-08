---
name: bug-hunt
description: 用户**显式**触发的 bug 主动猎查 skill——按 scope 扫一组文件 / crate / commit range，输出**带证据 + 置信度 + 严重度**的 bug 报告，不动业务代码。**只有**用户说 `/bug-hunt` 或显式说"扫一遍 X 找 bug / 帮我 audit X / X 这个 crate 有没有潜在 bug / 主动找 bug / bug hunt / 静态过一遍 X" 时才用。**不要**在用户问"为什么 X 不工作"时自动触发——那是 `debug-first` 的领地。**不要**在 PR review 时触发——那是 `code-review`。本 skill 专管"对一段没人喊救的代码主动 audit"。
allowed-tools: Bash, Read, Grep, Glob, Agent, Workflow
disable-model-invocation: true
---

# bug-hunt

> 触发：仅用户显式 `/bug-hunt` 或自然语言点名"扫 X 找 bug / audit X"
> 输出：带 file:line 证据 + 触发链 + 置信度 + 严重度 + 修复风险等级的 bug 列表
> 边界：**不动代码**——只产报告让用户拍板下一步走 PR 还是开 issue

## 为什么这个 skill 存在

LLM 主动找 bug 最常见的 3 个失败模式（社区共识 + 本仓踩过的坑）：

1. **幻觉 bug**："感觉这里有问题"但说不出具体证据 / 触发条件，用户去查发现是 LLM 误读上下文。本 skill **真实性闸门**强制每条 finding 必过 4 项交叉验证。
2. **报告噪音淹没真 bug**：把 nit / 风格 / 假设性优化全部当 bug 上报，用户筛 100 条找 1 条 critical。本 skill **置信度 × 严重度双轴**只报 (高置信 OR 高 severity)，nit 直接不报。
3. **盲扫整库无 scope**：让 LLM 自由发挥扫整个仓 → 结果跨域不深、误报多、用户没耐心读。本 skill **scope-first 强制**——scope 不清晰就先问用户，不瞎扫。

## 主流程（5 步顺序执行）

### Step 1: 收 scope（不清晰必须问）

scope 必须落到下面 4 类之一，**模糊就停下问用户**，不瞎扫：

| scope 类型 | 例子 | 适合什么 |
|---|---|---|
| **crate** | `cdt-watch` / `cdt-api` | 单一职责模块整审 |
| **files / glob** | `src-tauri/src/ipc/*` / `crates/cdt-ssh/src/connection*.rs` | 怀疑某文件群 |
| **commit range** | `main..HEAD` / `HEAD~10..HEAD` | 最近 N 次提交回顾 |
| **capability** | `file-watching` / `session-parsing`（落到 `openspec/specs/<cap>/spec.md`）| spec 维度审"行为 vs 实现" |

**禁止**：扫整个仓 / 扫"所有 Rust 代码" / scope 写成"找找看"。

scope 收到后做一次 sanity check：
```bash
# 估算扫描体量
find <scope> -name "*.rs" -o -name "*.svelte" -o -name "*.ts" 2>/dev/null | wc -l
# 大于 50 文件的 scope 提醒用户拆分
```

### Step 1.5: 路由决策（Workflow vs 直接扫）

根据 scope 体量选路径：

| scope 体量 | 路径 | 理由 |
|---|---|---|
| < 10 文件 | **直接扫**（现有 Step 2-5 流程） | 小 scope 不值得 workflow 的 agent spawn 开销 |
| 10-50 文件 | **调 Workflow**（`.claude/workflows/bug-hunt.js`） | 吃满 fan-out + schema + 上下文隔离收益 |
| > 50 文件 | 先提醒用户拆分（Step 1 已规定） | — |

**调 Workflow 的方式**：

```
Workflow({
  name: 'bug-hunt',
  args: {
    scope: '<收到的 scope 路径>',
    scopeType: '<crate|files|commit-range|capability>',
    riskLevel: '<low|medium|high>',   // 默认 high
    skipLenses: [<用户显式跳过的 lens id>]
  }
})
```

- `riskLevel` 决定跑多少 lens：`low` = 仅 L1+L2；`medium` = L1-L5（跳 L6）；`high` = 全部
- 默认 `riskLevel: 'high'`（用户没指定风险偏好时全跑）
- Workflow 返回结构化 `{ findings, openQuestions, discarded, metadata }`
- **lead 拿到返回后做 Step 5 报告输出**——Workflow 只产证据，综合判断由 lead 在完整上下文里做

**Workflow 失败降级**：若 Workflow 工具调用失败（返回 error / 用户拒绝 / runtime 不支持），**降级到直接扫流程**（Step 2-5），并告知用户"Workflow 不可用，降级为直接扫描"。

**实测指标记录**：每次 Workflow 跑完，在报告末尾附加：
- agent 总数 / input token / output token（从 workflow result 的 metadata 提取）
- finding 被采纳数（由后续用户交互确定，首次填 TBD）

### Step 2: 6 个 lens 分层扫（每个 lens 独立产出 candidate）

每个 lens 关注不同类型反模式。**默认全跑**；用户可显式跳过某 lens（如 "skip L5 安全" 因为是内部工具）。

详细反模式 checklist + grep recipe 在 `references/anti-patterns.md`，下面只列每个 lens 的**抓手**。

| Lens | 关注 | 主要抓手 |
|---|---|---|
| **L1 silent failures** | 吞异常 / 不可信路径 unwrap / fallback 掩盖 error / catch+log 不抛 | grep `unwrap()` / `expect(` / `let _ =` / `.ok()` 弃 Result / `catch.*continue` / `Err(_) =>` 后 default value |
| **L2 边界 + 状态机** | off-by-one / 空集合 / N=1 / 整数溢出 / 状态转移漏路径 / TOCTOU | grep `as usize` / `as u32` 强转 / `len() - 1` / `[0]` 不查空 / match 漏 arm 用 `_ =>` 兜 |
| **L3 并发 + 资源** | race / unbounded channel / unbounded cache / 未取消 task / 未退订 subscriber / blocking 调 in async | grep `Arc<Mutex` 加锁顺序 / `broadcast::channel(` capacity / `tokio::spawn` 无 cancel / `std::fs::` in async fn / `.lock().await` 长时间持有 |
| **L4 跨域契约** | IPC 字段名/类型 / serde 命名 / 跨 crate 公共 API breaking / 跨平台 / **外部 API 误用 / 协议违反 / doc-vs-impl drift** | grep `#[tauri::command]` 字段对齐 ui/ 消费 / `#[serde(rename_all` 是否漏 / `cfg(target_os` 漏分支 / `Path::is_absolute` (Windows 陷阱) / 比对 `///` 文档承诺 vs 函数实际行为 / 第三方库 API 调用看 [crates.io/ docs.rs](https://docs.rs) 看是否符合契约（如 `notify::Watcher::watch` 必须先 `watch` 再 drop） |
| **L5 安全** | 路径遍历 / 命令注入 / 不可信输入直接 `format!` 进 path/sql/cmd | grep `Command::new` 拼接用户输入 / `format!.*{}` 进 path / `..` 路径未规范 / 反序列化外部数据没 size 限制 |
| **L6 测试伪覆盖** | mock 替代真实路径 / 只测 happy / scenario 名对得上但行为没真覆盖 | 看 `#[cfg(test)]` 用了 `MockX` 不真起后端 / `assert!(true)` 占位 / scenario 名对应 test 函数但 assert 不验关键字段 |

每个 lens 产出 candidate 列表：`{反模式类型, 文件:行, 一行猜测}`。**这是粗筛——还没被认证**。

### Step 3: 真实性闸门（4 道必查 + 通过率决定置信度）

LLM 找 bug 最大风险是幻觉。**每条 candidate SHALL 走完 4 道交叉验证**——闸门**必查**，但**不必全过**：通过道数与最终能否进默认 Findings 直接挂钩（详 Step 4 双轴分级）。**任一闸门跳过没查 = 视同没过**。

#### Gate 1: Code evidence（必须能 quote）
- **要求**：能贴出 ≤ 10 行原代码 + 高亮指出反模式具体在哪一行
- **失败信号**：只能说"这个模块有问题"但贴不出代码 → 直接丢

#### Gate 2: Trigger path（必须能给具体输入/调用栈/时序）
- **要求**：能写出 "当用户做 X / 输入 Y / 在时序 T 时 → 走到这段代码 → 触发这个 bug"
- **失败信号**：只能说"理论上可能" → 降级为"推测"或丢
- **特例**：security bug 不需要真触发，只需"存在攻击路径"

#### Gate 3: Test gap check（grep 测试是否覆盖）
- **要求**：同时查 **standalone tests + inline tests**——Rust 常见 `#[cfg(test)] mod tests` 写在源文件里，光查 `tests/` 目录会漏一半。推荐 recipe：
  ```bash
  # 同时扫 tests/ 目录 + scope 内 inline #[cfg(test)] 块
  rg '<function_name>|<scenario>' <scope> tests/ 2>/dev/null
  # 进一步过滤到 inline test 块
  rg -B 50 '<function_name>' <scope> 2>/dev/null | rg '#\[cfg\(test\)\]|fn test_'
  ```
- **解释规则**：
  - 有测试 + **真 assert 关键字段** + 测试 pass → 大概率不是 bug，是约定行为（**降级或丢**）
  - 有测试 + 测试只跑 happy path 没覆盖 trigger 场景 → 升级置信度（**真 bug**）
  - 有测试 + assert 占位 / 不验关键字段 → L6 伪覆盖范畴，本身就是 bug
  - 无测试（含 inline + standalone 双查后都无） → 中性信号，看其他 gate
- **特例**：mock 测试不算覆盖（mock != 真后端，本仓踩过坑见 `e2e-http-verify` skill 引言）

#### Gate 4: Caller verify（跨文件查调用点）
- **要求**：grep 该函数的所有调用点，确认 bug 真能在调用方触发
  - `LSP findReferences` 优先（精确）
  - 退化用 `grep -rn 'fn_name'`
- **失败信号**：发现该函数只在测试 / 已废弃路径调用 → 降级或丢
- **常见误报源**：误读上下文 → 在 caller 那里参数已校验过 → 不是 bug

### Step 4: 双轴分级 + 过滤

每条过完 4 道闸门的 finding 打两个标签：

**置信度**（基于 Step 3 闸门通过道数）：
- `confirmed`（100% 确认）：4 道闸门**全过** + 触发链 1-2 步可复现
- `high`（≥ 80%）：4 道闸门**全过** + 触发链需特定时序/输入
- `medium`（≥ 50%）：3 道过，1 道有疑问 / 不能 100% 确认
- `low`（< 50%）：≤ 2 道过 / 含臆测成分

**严重度**（基于用户影响）：
- `critical`：数据丢失 / silent corruption / 安全漏洞 / panic 在用户主路径
- `major`：用户可见行为错 / 性能严重劣化 / 功能不完整
- `minor`：edge case 错 / 错误信息不准 / 资源未释放（短期可恢复）
- `nit`：风格 / 可读性 → **从来不报**

**报告过滤规则**（与 Step 3 "必查不必全过" 对齐——只有 4 道全过的才能进默认 Findings）：

| 置信度 \ 严重度 | critical | major | minor | nit |
|---|---|---|---|---|
| `confirmed` | ✅ Findings | ✅ Findings | ✅ Findings | 丢 |
| `high` | ✅ Findings | ✅ Findings | ✅ Findings | 丢 |
| `medium` | 🔵 开放问号 | 🔵 开放问号 | 🔵 开放问号 | 丢 |
| `low` | 🔵 开放问号 | 🔵 开放问号 | 丢 | 丢 |

- ✅ **Findings**：进默认报告主体
- 🔵 **开放问号**：进报告的"开放问号"段，标"< 4 道闸门 / 低置信"让用户决定是否深 dig，**不**算确认 bug
- **丢**：完全不写进报告

**关键不变量**：默认 Findings 段里不会出现 `medium` / `low` 置信度——它们一律落到"开放问号"，避免 medium/minor 噪音淹没真 bug。

### Step 5: 出报告（固定模板）

报告 5 段结构（**严格按此结构出**）：

```markdown
# Bug Hunt Report — <scope>

## 概览
- 扫描 scope: <具体>
- 跑了哪些 lens: L1 / L2 / L3 / L4 / L5 / L6
- 候选总数: N → 4 道闸门全过的: M（critical: a / major: b / minor: c）→ 进 Findings: M
- 转开放问号（≤ 3 道闸门过）: K
- 跳过的 lens（含理由）: ...

## Findings（仅 confirmed / high 置信）

### Bug 1 — [critical/major/minor] · [confirmed/high] · [Lx]

**位置**: `file:line`
**反模式**: <一行点名>

**代码证据**:
\`\`\`rust
// file:line（贴上下文 ≤ 10 行）
let foo = bar.unwrap();  // 反模式行
\`\`\`

**触发链**:
1. 用户做 X / 输入 Y
2. 走到 `caller_fn` (file:line)
3. 调用 `bar()` 返回 None
4. unwrap → panic

**影响**: <用户可见现象 / silent 数据丢失 / panic / 安全后果>

**测试缺口**: <grep 结果——有覆盖 / 只覆盖 happy path / 完全无测试>

**修复风险等级**: 一行 / 多文件 / 需 openspec

---

### Bug 2 ...

## 开放问号（≤ 3 道闸门过 / medium / low 置信）
- 怀疑点 A（≤ 3 道过：缺 Gate X）：[file:line + 一行猜测 + 缺哪道闸门没过]
- 怀疑点 B：...

## 跳过 / 未覆盖说明
- 未跑 L5 安全：本工具内部，无外部输入面
- 跳过 file X：超过 100KB 按用户全局 preference 跳过
```

## 真实 vs 幻觉的分水岭（关键自检题）

写完每条 finding 在脑里过这 5 题。**任一答 "无法回答 / 含糊"** → 降置信度或丢：

1. 我能贴出 ≤ 10 行原代码并指出第几行是 bug 吗？
2. 我能用 1-2 句具体场景描述何时触发吗？（不是"理论上可能"）
3. 我 grep 过测试目录吗？该路径在测试里是 pass 还是无覆盖？
4. 我看过 caller 吗？参数在调用方是否已被校验过？
5. 用户拿这条 finding 去找同事 review，同事会同意还是反问"你哪儿看的"？

## 复用本仓已有 reviewer agents（按域调）

scope 命中下面情况时 SHALL 用 `Agent` tool 派发对应 reviewer 做并行二审。**reviewer 的命中是 Gate 1（code evidence）的第三方背书**——可让 Gate 1 在原本"我自己看着像但不太确定"时升级为通过，**但不替代 Gate 2 / 3 / 4**：

- ✓ reviewer 命中 + 4 道 gate 自查全过 → 进 Findings（confirmed / high）
- ✓ reviewer 命中 + 仅 3 道 gate 过 → **仍然落开放问号**（reviewer 不绕过 4 gate 全过的硬约束）
- ❌ reviewer 命中 + ≤ 2 道 gate 过 → 丢

**关键不变量**：reviewer 升级**只动严重度的优先排序**（critical 在前 / 同 severity reviewer 命中的排前），**不动置信度**——置信度只由 4 gate 通过道数决定。这条铁律消除了 medium 候选靠 reviewer 钩子升 high 进 Findings 的后门。

| scope 涉及 | 调哪个 agent | 何时调 |
|---|---|---|
| Rust 改动（任何 crate） | `rust-conventions-reviewer` | L1 / L3 lens 完成后并行调，比对 |
| Windows 兼容性敏感（路径 / fs / home dir） | `windows-compat-reviewer` | L4 lens 完成后调，专抓本仓踩过的 windows 反模式 |
| `src-tauri/` 配置 / IPC | `tauri-config-reviewer` | scope 含 `src-tauri/` 时调 |
| Svelte 组件 | `ui-reviewer` | scope 含 `ui/src/` 时调 |
| openspec capability 审计 | `spec-fidelity-reviewer` | scope 是 capability 时调，看 scenario 是否真有 test |

**并行**：6 个 lens + N 个 reviewer 同一 message 多 Agent tool call 派发，省 wall time。

**禁止**：在 reviewer 没产出前提交 finding——其结论可能反转 candidate 的置信度。

## 与其他 skill / agent 的边界（关键）

| 工具 | 触发场景 | 与 bug-hunt 的区别 |
|---|---|---|
| **bug-hunt**（本 skill） | 主动找未知 bug（"扫一遍 X / audit X"） | 显式触发 / 无确定 bug / 出报告不动代码 |
| `debug-first` | 已知有 bug 要排查（"X 不工作 / 还是有问题"） | 自动触发诊断信号词 / 已有问题现象 |
| `code-review` 系列 | review PR diff | 限于 diff 范围，不主动扫无改动代码 |
| `verify` | 验证某改动是否真 work | 改动后的功能确认，不是找 bug |

判断不准：用户是否说出具体现象？说了 → `debug-first`；没说"扫"或"audit" → 不是本 skill 的活。

## 输出后用户的下一步

报告交付后**不要主动开 Issue / 改代码**，让用户决定。典型分流：

- `confirmed/critical` 单点修 → 用户可能直接说"修 Bug 1"，那就走 `preflight` + 常规 PR
- `confirmed/major` 跨域 / 改 IPC 字段 → 走 openspec（`/opsx:propose`）
- `medium` 置信 → 用户可能让你"再 dig 一下 Bug 3"，加测试复现或 codex 二审
- 多个相关但非主线 → 按 `CLAUDE.md::遗留事项归宿` 开 GitHub Issue 默认 `bug` label

**禁止**：报告里写"我已经修了 Bug 1"——本 skill 不动代码。

## 反 hallucination 速查（再强调一遍）

写报告时反复用以下三个反例自校：

❌ **空想 bug**："这里可能有 race condition" + 没说哪两个线程怎么交错 → 丢
❌ **不存在的代码**：贴了一段代码但 grep 仓里找不到 → 丢（你在编代码）
❌ **泛化的反模式指控**："这个模块到处都有 unwrap" → 列具体 file:line，不能列就丢

✓ **可信 bug**：file:line + 代码引文 + 触发链 + 影响 + 测试缺口都齐全。

## 速参引用

- 详细反模式 checklist + 各 lens grep recipe：`references/anti-patterns.md`
- 误报防御场景（哪些"看起来像 bug"其实不是）：`references/false-positives.md`
- 本仓已踩过的反模式合集（perf / windows / silent-failure / IPC 字段）：`.claude/rules/perf.md` + `crates/CLAUDE.md` + `src-tauri/CLAUDE.md`
- bug 报告 → 后续推进节拍：`.claude/rules/opsx-apply-cadence.md`

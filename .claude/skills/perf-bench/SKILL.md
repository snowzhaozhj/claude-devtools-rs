---
name: perf-bench
description: 大会话 IPC 性能基准跑分 + 解读。跑 `cdt-api/tests/perf_get_session_detail.rs`，解析后端各阶段耗时 + 字段级 payload breakdown + raw vs IPC OMIT 对比，按"6.5 KB/ms（含 V8 JSON.parse 端到端）"和"13 KB/ms（纯字节）"两套吞吐估算 IPC 时间，给出"瘦身/不瘦身"verdict。**只要**用户提到卡顿 / 性能 / 慢 / 大会话 / payload / IPC 慢 / SessionDetail 首屏 / 加载耗时 / 风扇起转，或显式 `/perf-bench`，**都用这个 skill** 作为首选诊断入口——不要自己手跑 cargo test 后乱解读数字。
---

# perf-bench

claude-devtools-rs 把 SessionDetail 首屏性能列为硬约束。任何"为什么慢 / 卡 / 卡顿 / payload 大"的诊断从此 skill 起步——它把"跑 bench + 解读 stdout + 估算 IPC 时间 + 对照 verdict 阈值"标准化，避免每次手算重复出错。

## 路径与命令

- 工作目录：项目根（`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/`）
- 命令：

  ```bash
  cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
  ```

- 样本：`crates/cdt-api/tests/perf_get_session_detail.rs::let samples = [...]`（硬编码 session id 列表）。新增样本要改测试文件后重跑 release 编译——不是热加载。
- 找不到 `~/.claude/projects/` 或对应 project 时测试静默跳过（输出 `跳过：...`）；不视为失败。

## 工作步骤

### 1. 环境前置检查

- `~/.claude/projects/-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs/` 存在 → 继续；否则报"无样本可跑"并退出。
- 不必先 `cargo build`——`cargo test --release` 会处理。

### 2. 跑 bench（一次 Bash，timeout ≥ 240000）

```bash
cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture 2>&1 | tail -30
```

### 3. 解析 stdout

每个样本输出 3 行（顺序固定）：

```
<sid>: msgs=<N> chunks=<C> subs=<S> | parse <P>ms | scan_subs <SS>ms (parse <SP>ms + chunk <SC>ms) | build <B>ms | serde <SE>ms (<RAW> KB) | TOTAL <T>ms
  payload breakdown: tool_output=<TO> KB, tool_input=<TI> KB, subagent_msgs=<SM> KB, response_content=<RC> KB, semantic_steps=<SS2> KB, other≈<OT> KB
  ★ get_session_detail (with OMIT): payload=<IPC> KB, ipc <I> ms
```

### 4. 估算 + 对比

对每个样本算：
- `节省% = (RAW - IPC) / RAW * 100`
- `est_ipc_e2e_ms = IPC / 6.5`（**端到端**，含 webview JSON.parse 反序列化——这是用户感知的"点开 session 到画面出来"的时间）
- `est_ipc_wire_ms = IPC / 13`（**纯网络字节**，不含 parse；仅作为吞吐上限参考）
- `subagent_msgs 占比 = SM / RAW * 100`（判断是否 phase 2 类裁剪有效）

**两套吞吐都报**——用户问"为什么慢"时关心的是 e2e；用户问"网络带宽够不够"时关心的是 wire。CLAUDE.md "IPC payload 瘦身模式"条已固化两个数字的来源（macOS Tauri 实测）。

### 5. Verdict 阈值（按当前 IPC payload 与 e2e 估算）

| IPC payload | est e2e (6.5 KB/ms) | Verdict | 建议 |
|-------------|---------------------|---------|------|
| < 1024 KB | < 160 ms | ✅ 已优化到位 | 无需进一步动 |
| 1024–3072 KB | 160–470 ms | ⚠ 可接受，监测中 | 不主动优化，记录基线 |
| ≥ 3072 KB | ≥ 470 ms | ❌ 仍是瓶颈 | 找下一大头字段（看 breakdown），按 `subagent-messages-lazy-load` 模式做新一轮裁剪 |

跨过 470 ms 的 e2e 估算就值得动手——用户感知"明显卡"通常在 300-500 ms 起。

### 6. 输出报告（≤ 400 字 + 1 张表）

```markdown
## perf-bench 结果

| session | msgs | chunks | subs | RAW | IPC OMIT | 节省 | est e2e | est wire |
|---------|------|--------|------|-----|----------|------|---------|----------|
| <sid 短 8 位> | N | C | S | <RAW>KB | <IPC>KB | <%> | ~<a>ms | ~<b>ms |

**后端各阶段**（最大样本）：parse <P>ms / scan_subs <SS>ms / build <B>ms / serde <SE>ms / TOTAL <T>ms

**Payload breakdown**（最大样本）：subagent_msgs <SM>KB (<%>) / response_content <RC>KB / tool_output+input <TO+TI>KB / other <OT>KB

**Verdict**：<✅/⚠/❌> ...

**下一步**：<具体 action 或 "无需动作">
```

## 硬性约束

- **只读 + 跑命令**：不改任何代码、不改 samples 数组、不动 TS_BASELINE_DEVIATIONS.md / CLAUDE.md。
- **不能编造数字**：每个数字必须能从 stdout 找到对应行；解析失败就如实说"输出格式不符预期，请检查测试文件"。
- **两套吞吐都报**：6.5（含 parse e2e）+ 13（纯字节），不要只挑一个——CLAUDE.md "IPC payload 瘦身模式"条已说明区别。未来如换平台需重新校准两个数字。
- **不主动启动 `just dev`**：bench 不依赖 UI；要前端实测让用户自己跑 `just dev` 看 console `[perf]` 探针。
- **样本不存在不报错**：`跳过：...` 是合法输出；视为"环境不齐"友好告知。
- **解读引用主 spec**：建议下一步优化时只引用 `openspec/specs/<cap>/spec.md` Requirement 名 + change slug；**不要**写 `archive/2026-...` 路径（CLAUDE.md spec 变更约定第 3 条）。
- **冷启动 / 列表性能不在本 skill 范围**：那条路径走 `cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture`（参考 `.claude/rules/perf.md`），本 skill 只管 SessionDetail 首屏。

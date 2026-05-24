# SessionDetail 首屏 bench 跑分

由 [`perf` SKILL.md](../SKILL.md) 路由到此（用户描述含"首屏 / 加载耗时 / payload / IPC 慢 / 大会话 / SessionDetail"）。

## 路径与命令

- 工作目录：项目根
- 命令：`cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture`
- 样本：`crates/cdt-api/tests/perf_get_session_detail.rs::let samples = [...]`（硬编码 session id 列表）
- 找不到 `~/.claude/projects/` 或对应 project 时静默跳过；不视为失败

## 工作步骤

### 1. 环境前置检查

`~/.claude/projects/-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs/` 存在 → 继续；否则报"无样本可跑"并退出。

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

每个样本算：
- `节省% = (RAW - IPC) / RAW * 100`
- `est_ipc_e2e_ms = IPC / 6.5`（**端到端**含 webview JSON.parse 反序列化，用户感知"点开 session 到画面出来"）
- `est_ipc_wire_ms = IPC / 13`（纯网络字节，吞吐上限参考）
- `subagent_msgs 占比 = SM / RAW * 100`（判断 phase 2 类裁剪是否有效）

**两套吞吐都报**——用户问"为什么慢"关心 e2e；问"带宽够不够"关心 wire。

### 5. Verdict 阈值

| IPC payload | est e2e (6.5 KB/ms) | Verdict | 建议 |
|---|---|---|---|
| < 1024 KB | < 160 ms | ✅ 已优化到位 | 无需进一步动 |
| 1024–3072 KB | 160–470 ms | ⚠ 可接受，监测中 | 不主动优化，记录基线 |
| ≥ 3072 KB | ≥ 470 ms | ❌ 仍是瓶颈 | 看 breakdown 找下一大头，按 IPC payload 瘦身模式裁剪 |

跨过 470 ms 值得动手——用户感知"明显卡"通常 300-500 ms 起。

### 6. 输出报告（≤ 400 字 + 1 张表）

```markdown
## perf-bench 结果

| session | msgs | chunks | subs | RAW | IPC OMIT | 节省 | est e2e | est wire |
|---|---|---|---|---|---|---|---|---|
| <sid 短 8 位> | N | C | S | <RAW>KB | <IPC>KB | <%> | ~<a>ms | ~<b>ms |

**后端各阶段**（最大样本）：parse <P>ms / scan_subs <SS>ms / build <B>ms / serde <SE>ms / TOTAL <T>ms
**Payload breakdown**：subagent_msgs <SM>KB (<%>) / response_content <RC>KB / tool_output+input <TO+TI>KB / other <OT>KB
**Verdict**：<✅/⚠/❌> ...
**下一步**：<具体 action 或"无需动作">
```

## 硬性约束

- 只读 + 跑命令：不改代码 / samples 数组 / 任何 .md
- 每个数字必须能从 stdout 找到对应行；解析失败如实说"输出格式不符预期"
- 两套吞吐都报，不挑一个
- 不主动启动 `just dev`；前端实测让用户自己跑
- 样本不存在不报错——`跳过：...` 是合法输出
- 冷启动 / 列表性能走 `cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture`（不在本 reference 范围）

---
name: perf-bench
description: 跑大会话 IPC 性能基准（cdt-api/tests/perf_get_session_detail.rs），解析后端各阶段耗时 + 字段级 payload breakdown + raw vs IPC OMIT 对比，按 13 KB/ms 估算 IPC 时间，给出"瘦身/不瘦身"verdict。用户提"卡顿/性能/慢/大会话/payload"或显式 `/perf-bench` 时触发。
---

# perf-bench

当用户提到"卡顿 / 性能 / 慢 / 大会话 / IPC payload"或显式 `/perf-bench` 时触发——这是 SessionDetail 首屏性能问题的**首选诊断入口**。

## 路径与命令

- 工作目录：项目根（`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/`）
- 命令：

  ```bash
  cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
  ```

- 该测试样本在 `crates/cdt-api/tests/perf_get_session_detail.rs` 的 `let samples = [...]` 里硬编码（当前 3 个 session id）。**新增样本要改测试文件后重跑 release 编译**——不要假设可以热添加。
- 找不到 `~/.claude/projects/` 或对应 project 时测试静默跳过（输出 `跳过：...`）；不视为失败。

## 工作步骤

1. **环境前置检查**
   - `~/.claude/projects/-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs/` 存在 → 继续；否则报告"无样本可跑"并退出。
   - 不必先 `cargo build`——`cargo test --release` 会处理。

2. **跑 bench**（一次调用 Bash，timeout 至少 240000）

   ```bash
   cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture 2>&1 | tail -20
   ```

3. **解析 stdout**

   每个样本输出 3 行（顺序固定）：

   ```
   <sid>: msgs=<N> chunks=<C> subs=<S> | parse <P>ms | scan_subs <SS>ms (parse <SP>ms + chunk <SC>ms) | build <B>ms | serde <SE>ms (<RAW> KB) | TOTAL <T>ms
     payload breakdown: tool_output=<TO> KB, tool_input=<TI> KB, subagent_msgs=<SM> KB, response_content=<RC> KB, semantic_steps=<SS2> KB, other≈<OT> KB
     ★ get_session_detail (with OMIT): payload=<IPC> KB, ipc <I> ms
   ```

4. **估算 + 对比**

   对每个样本算：
   - `节省% = (RAW - IPC) / RAW * 100`
   - `est_ipc_ms = IPC / 13`（Tauri webview 实测吞吐 ≈ 13 KB/ms；CLAUDE.md "陷阱" 段已固化）
   - `subagent_msgs 占比 = SM / RAW * 100`（用于判断是否 phase 2 类裁剪有效）

5. **Verdict 阈值**（按当前 IPC payload）

   | IPC payload | Verdict | 建议 |
   |-------------|---------|------|
   | < 1024 KB | ✅ 已优化到位 | 无需进一步动 |
   | 1024–3072 KB | ⚠ 可接受，监测中 | 不主动优化，记录基线 |
   | ≥ 3072 KB | ❌ 仍是瓶颈 | 找下一大头字段（看 breakdown），按 `subagent-messages-lazy-load` 模式做新一轮裁剪；参考 `openspec/followups.md` "性能 / 首次打开大会话卡顿" 条目里"剩余瓶颈"段 |

6. **输出报告**（≤ 400 字 + 1 张表）

   结构：

   ```markdown
   ## perf-bench 结果

   | session | msgs | chunks | subs | RAW | IPC OMIT | 节省 | est IPC ms |
   |---------|------|--------|------|-----|----------|------|-----------|
   | <sid 短 8 位> | N | C | S | <RAW>KB | <IPC>KB | <%> | ~<n>ms |

   **后端各阶段**（最大样本）：parse <P>ms / scan_subs <SS>ms / build <B>ms / serde <SE>ms / TOTAL <T>ms

   **Payload breakdown**（最大样本）：subagent_msgs <SM>KB (<%>) / response_content <RC>KB / tool_output+input <TO+TI>KB / other <OT>KB

   **Verdict**：<✅/⚠/❌> ...

   **下一步**：<具体 action 或 "无需动作">
   ```

## 硬性约束

- **只读 + 跑命令**：不改任何代码、不改 `samples` 数组、不动 followups.md / CLAUDE.md。
- **不能编造数字**：每个数字必须能从 stdout 找到对应行；解析失败就如实说"输出格式不符预期，请检查测试文件"。
- **吞吐系数硬编码 13 KB/ms**：来自 Tauri webview macOS 实测（首次定位时 user 上报的 console `[perf]` 数据），未来如换平台需重新校准——使用时声明"按 13 KB/ms 估算"。
- **不主动启动 `just dev`**：bench 不依赖 UI；要前端实测让用户跑 `just dev` 自己看 console `[perf]` 探针。
- **样本不存在不报错**：`跳过：...` 是合法输出；视为"环境不齐"友好告知。
- **解读引用主 spec**：建议下一步优化时只引用 `openspec/specs/<cap>/spec.md` Requirement 名 + change slug；**不要**写 `archive/2026-...` 路径（CLAUDE.md L138-141 的 4 条硬约束）。

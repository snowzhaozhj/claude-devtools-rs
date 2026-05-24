# idle CPU 系统性诊断

由 [`perf` SKILL.md](../SKILL.md) 路由到此（用户描述含"idle CPU 高 / 风扇 / 后台烧 / 偶尔卡 / 线程多 / 进程占 X%"）。

## 9 步诊断节拍

1. **基线**：Activity Monitor 看 4 件套（CPU% / 线程数 / 闲置唤醒 / 累计 CSW），跟 `.claude/rules/perf.md::性能预算` 对比
2. **找 PID + 区分 build**：`pgrep -f claude-devtools-tauri` + `ps -p $PID -o command`。`/Applications/...` = release；`cargo tauri dev` = debug（debug 自动开 devtools 贡献额外 5-10% CPU，结论时务必区分）
3. **抓栈**：`bash .claude/skills/perf/scripts/sample-cpu.sh <PID> 30` 一次跑 sample + top + 栈分类
4. **趋势**：看脚本输出的 `top.txt`——CPU% 是稳态还是 spike；线程数是动态变化（fan out + 销毁循环）还是稳定
5. **栈分类**：看 `stack-classification.txt`——`thread join calls + ulock_wait` 在 30s 内 > 50 说明 blocking pool 周期性 create/destroy
6. **回归归因**：用户给基线版本（"v0.X 时还正常"）→ `git log v0.X..main --oneline --no-merges` 筛可疑 commit；锁定的用 `git blame` 验引入时间
7. **关联现有 issue**：`gh issue list --label performance` 看是否已记录
8. **codex 渐进多轮二审**：复杂诊断走 4 阶段进路，模板见 `.claude/templates/codex-prompt-progressive-diagnosis.md`
9. **ROI 决策矩阵**：完整问题清单出来后给用户 P0/P1/P2/P3 优先级表

## 容易踩的坑（v0.5.6→v0.5.8 诊断学到的）

- **线程数 ≠ CPU 高**：condvar wait 线程不烧用户态 CPU；spike 后残留 10s 才被销毁
- **sample idle 时刻 ≠ active 时刻**：30s 长 sample 才能捕获 spike 内栈分布；短 sample idle 时刻抓不到真热点
- **`pthread_join` 风暴 ≠ 死循环**：是 blocking pool 周期性销毁尾部（每 keep_alive 周期一次）；缩短 keep_alive 是负优化，应延长

## 输出报告模板

```markdown
## idle CPU 诊断结果（PID <X>，<release/dev>）

**基线对比**（vs `.claude/rules/perf.md` 预算）：
- CPU% 主进程：X% / WebView：Y%（预算 < 2%）
- 线程数峰值：N（预算 < 50）
- CSW 30s delta：M（预算 < 100/s）

**热点栈 Top 3**（去 idle wait）：
1. <frame> N 次
2. ...

**根因假设**（标置信度）：
- 100% 确认：<...>
- 高怀疑 60%：<...>
- 推测：<...>

**回归归因**（如有）：v0.X → v0.Y 之间 commit #abc 引入

**修复方案**（按 ROI 排序）：
| Priority | Item | Impact | Effort | Risk | Action |

**下一步**：<具体 action / 或建议起 codex 渐进二审进一步诊断>
```

## 硬性约束

- 只读 + 跑命令：不改任何代码
- 每个数字能从 sample/top 输出找到对应行；解析失败如实说"输出格式不符预期"
- 不主动 merge / push / 起 PR / 起 bg：destructive shared state 留用户拍板

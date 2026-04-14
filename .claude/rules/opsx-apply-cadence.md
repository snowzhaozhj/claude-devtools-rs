# opsx:apply 推进节拍（硬约束）

port 内任何多步改动必须按固定流水线推进，**不得**把 PostToolUse clippy hook 的沉默当作"可以停手"的信号。

## 节拍

1. `Edit` 源文件（可并行）
2. `cargo clippy --workspace --all-targets -- -D warnings` 汇总校验（**不是**靠 hook 单文件回显）
3. `cargo fmt --all`
4. `cargo test -p <crate>`（或 `--workspace`）
5. `npm run check --prefix ui`（如改了 `ui/` 下的文件）
6. `openspec validate <change> --strict`（如有 openspec change）
7. 勾 `openspec/changes/<change>/tasks.md` 的 checkbox
8. 发最终文本总结

## 自检规则

- 每轮 tool call 结束前自检："这批之后要么发下批工具、要么发最终文本，二者必居其一"
- 只发 Edit 没有后续计划 = 禁止
- 开工时把 tasks.md 的每个 `##` section 作为 `TaskCreate` 入队
- 完成一个 `TaskUpdate completed` 一个，给自己留显式的"下一步指针"

# 有问题的 spec 内容（应有违规）

## 过期锚点

修复见 commit abc1234567890abcdef1234567890abcdef123456。

参考 PR #123 的讨论。

具体实现在 `a1b2c3d` 提交。

代码位于 L42-L50。

## 精确源路径

实现位于 crates/cdt-parse/src/state_machine.rs 的 parse 函数。

前端渲染逻辑在 src-tauri/src/commands/session.rs 中。

Store 实现见 ui/src/stores/sessionStore.ts 文件。

## 实测数据

实测冷启动耗时 95ms。

bench 结果显示 74ms wall time。

measured latency was 120ms in production.

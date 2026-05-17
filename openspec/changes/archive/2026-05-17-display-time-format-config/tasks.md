## 1. `cdt-config` 后端字段与持久化

- [x] 1.1 在 `crates/cdt-config/src/types.rs` 定义 `pub enum TimeFormat { #[serde(rename = "24h")] H24, #[serde(rename = "12h")] H12 }`，加 `Default` 实现返回 `H24`
- [x] 1.2 `DisplayConfig` 新增 `pub time_format: TimeFormat` 字段，标 `#[serde(default)]` 保证旧配置文件缺字段时反序列化为 `TimeFormat::H24`
- [x] 1.3 在 `crates/cdt-config/src/manager.rs::update_display` 加 `"timeFormat"` match 分支：解析 `v.as_str()` 为 `"24h"` / `"12h"`，非法值返回 validation error（错误信息含字段名 `timeFormat`）
- [x] 1.4 跑 `cargo test -p cdt-config` 确认现有测试不破，必要时补 `update_display` 处理 `timeFormat` 的单测

## 2. `cdt-api` IPC contract round-trip

- [x] 2.1 在 `crates/cdt-api/tests/ipc_contract.rs` 加 `update_config_display_time_format_round_trip` 测试：(a) 默认 `getConfig` 断言 `display.timeFormat == "24h"`；(b) 改 `"12h"` + 断言；(c) 改回 `"24h"` + 断言；(d) `"bogus"` expect_err 且错误含 `timeFormat`；(e) 空字符串同样拒绝
- [x] 2.2 跑 `cargo test -p cdt-api --test ipc_contract` 全绿

## 3. UI 类型与格式化工具

- [x] 3.1 `ui/src/lib/api.ts` 的 `DisplayConfig` interface 新增 `timeFormat: "24h" | "12h"`，并导出 `type TimeFormat = "24h" | "12h"`
- [x] 3.2 `ui/src/lib/__fixtures__/config.ts`（与其他 fixture 含 displayConfig 的文件）默认值同步 `timeFormat: "24h"`
- [x] 3.3 `ui/src/lib/formatters.ts` 新增 `export function formatClock(date: Date | number, hour12: boolean): string`，内部走 `toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12 })`
- [x] 3.4 `ui/src/lib/__tests__/formatters.test.ts`（或同目录现有测试文件）补 `formatClock` 单测：固定 timestamp 在 24h 模式产 `\d{2}:\d{2}:\d{2}` 不含 "上午/下午"；12h 模式含 "上午" 或 "下午"

## 4. SessionDetail 接入

- [x] 4.1 `ui/src/routes/SessionDetail.svelte` 删除内联 `ftime` 的硬编码 `hour12: true`，改 `import { formatClock } from "$lib/formatters"`
- [x] 4.2 把 `ftime(d)` 替换为 `formatClock(d, config.display.timeFormat === "12h")`（或等价闭包），保证 SettingsView 改 config 后渲染即时切换
- [ ] 4.3 桌面手动验证：`just dev` 启动后切 SettingsView · 显示 · 时间格式，切 `12h` 看 SessionDetail 时间戳变 "上午 X 点 XX 分 XX 秒"；切回 `24h` 变 `HH:MM:SS`

## 5. SettingsView 设置项

- [x] 5.1 `ui/src/routes/SettingsView.svelte` Display 区段加 `SettingsField label="时间格式" description="切换 24 小时制 / 12 小时制（带上午/下午）"`，control snippet 为 `<select>` 含 `24h` / `12h` 两 option
- [x] 5.2 `onchange` 调 `updateDisplay("timeFormat", value)`，乐观更新本地 `$state` 再异步调 API，失败时回滚（与现有 fontSans / fontMono 同模式）

## 6. 验证

- [x] 6.1 跑 `just preflight`（fmt + lint + test + spec-validate）全绿
- [x] 6.2 跑 `pnpm --dir ui run check` 单独确认 svelte-check 无报错
- [x] 6.3 `openspec validate display-time-format-config --strict` 通过

## 7. 发布

- [ ] 7.1 push 分支 + 开 PR
- [ ] 7.2 wait-ci 全绿
- [ ] 7.3 codex 二审通过（如发现 bug：修 → push → 回到 7.2 重跑；可循环 M 次）
- [ ] 7.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

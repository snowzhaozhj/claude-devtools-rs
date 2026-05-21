---
name: e2e-http-verify
description: 用 cdt-cli HTTP server + vite proxy + 浏览器 `?http=1` 入口跑端到端真数据验证 backend / 前端行为类修法。**只要**用户说"用 cli 起 server / 用 http 接口验证 / 浏览器自动化测试 / 真数据 e2e / chrome devtools 端到端 / 验证修法 / 用户感知 bug 复现 / 桌面端看不到修复 / sidebar 显示异常 / 切 group 慢 / 列表渲染 / 卡顿"或显式 `/e2e-http-verify`，**都用这个 skill**。**agent 在以下时机也 SHALL 自动调用**（无需用户念关键词）：(a) 改完任何 IPC 字段 / 后端算法 / 状态机 / SSE 路径准备宣告完成前；(b) 改完 Svelte 组件渲染 / store 转换 / IPC 字段消费类前端行为 + vitest/playwright 通过后；(c) PR push 前的最终验证一环；(d) 用户报"我重启了桌面端还是没修好"。避免每次重新摸 vite proxy / SSE prelude / BrowserTransport 路由 / 端口冲突 / chrome-devtools mcp evaluate_script 陷阱，也避免修完只跑 unit test + mockIPC 就声称完成（mockIPC fixture ≠ 真后端数据，桌面端 binary 不一定用上新代码）。
---

# e2e-http-verify

修 backend / Svelte 组件 / IPC 字段消费等**任何行为类**改动后，在真浏览器跑真后端看一遍再宣告完成。unit test + mockIPC 只验单点，端到端只能这里验。

**只在 dev 调试场景用**（不影响 release bundle）。用户桌面 Tauri app 走 IPC 不走 HTTP server，本 skill 验证通过 ≠ 用户桌面端自动修好——结尾要明确告诉用户重启 `cargo tauri dev` 才能看到修复。

## 何时用 / 何时跳

| 用 | 跳 |
|---|---|
| backend 改 IPC 字段 / cursor / cache / metadata / list_sessions / SSE | 改 Tauri 专属 API（通知 / 托盘 / setBadgeCount）—— HTTP transport 不暴露 |
| 前端改 sidebar / Sessions 列表 / SessionDetail / store 状态机 / IPC 消费 | 纯样式微调（颜色 / 间距）—— vitest snapshot + 人眼看截图够了 |
| 用户报 sidebar / 切 group / 切 worktree / 列表卡顿 | 改 nothing 只是"看看效果" → 直接 chrome-devtools mcp 打开桌面 app |
| PR push 前 final smoke | 改的是只在 Tauri runtime 用的 IPC（HTTP 路由没暴露）|

## Quick Start

```bash
# 起 cdt-cli + vite + 健康检查（首次 build 30-90s，incremental ~10s）
bash .claude/skills/e2e-http-verify/scripts/start.sh

# 收尾（不动桌面 Tauri app）
bash .claude/skills/e2e-http-verify/scripts/stop.sh
```

`start.sh` 把"端口归属判定 / cdt-cli 起 / vite 起 / proxy 健康检查"封进去。看到 `✓ Ready` 才进 Step 2。

**复用判断**（上一轮没收尾 + 代码没改时跳冷启）：

```bash
lsof -iTCP:3456 -sTCP:LISTEN && lsof -iTCP:5173 -sTCP:LISTEN && \
  curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:5173/api/projects
# 三件都 OK + 200 → 直接跳 Step 2/3 省 10s
```

**改了代码必须**：
- 改 `vite.config.ts` → `pkill -f vite` 重起（vite config 不 hot reload）
- 改 backend Rust → `pkill -f 'target/(debug|release)/cdt( |$)'` + 重跑 start.sh（否则跑旧 binary）
- 改 Svelte → vite HMR 自动；reload 浏览器即可
- 改 main.ts / transport.ts → 浏览器 full reload（`navigate type=reload ignoreCache=true`）

## Step 2: 后端 HTTP 探针

**先 JIT 查 schema 再 curl**——SKILL 不抄字段名（API schema 会漂移）。直接跑：

```bash
bash .claude/skills/e2e-http-verify/scripts/probe.sh           # 列 projects + groups（含真字段 keys）
bash .claude/skills/e2e-http-verify/scripts/probe.sh --schema  # 完整 schema 第一条
bash .claude/skills/e2e-http-verify/scripts/probe.sh --routes  # grep cdt-api routes.rs 列所有 axum route
bash .claude/skills/e2e-http-verify/scripts/probe.sh --cache-miss claude-devtools-rs
                                                               # 重启 cdt-cli 强制 cache miss
                                                               # → curl 立即 skeleton → sleep 6s → curl title fill
```

**cache miss 复现要重启 cdt-cli**——上一轮跑过 cache 就暖了，直接 curl 会看到 title fill 误判 bug。`--cache-miss` 自动处理重启 + 时序。

调任意 IPC：先 `--routes` 拿真路径，再 `curl $BE/api/<path>?... | python -m json.tool` 看 keys，不盲拼字段名。

## Step 3: 浏览器 e2e（chrome-devtools mcp）

URL 固定 `http://127.0.0.1:5173/?http=1`。`?http=1` 在 `ui/src/main.ts` 跳过 mockIPC 让 BrowserTransport 接管真后端。

**两条路径，按需混用**：

### 路径 A：`window.__cdtTest` 绕过 sidebar（推荐用于点开特定 session）

```js
// 在 chrome-devtools mcp evaluate_script 里
window.__cdtTest.openTab(sessionId, projectId, 'label')
// 注意参数顺序：sessionId 在前、projectId 在后（反了报 Cannot read 'length'）
```

`?http=1` 入口下 `__cdtTest` 已注入（main.ts 改动落地后，无 helper 是旧版本未拉取）。所有方法：`openTab / openSettingsTab / openNotificationsTab / openMemoryTab / setActiveTab`。

### 路径 B：snapshot + click（点 sidebar 模拟用户操作）

```
navigate_page  url=http://127.0.0.1:5173/?http=1
wait_for       text=["项目", "Sessions"]   # 等 hydrate
take_snapshot                              # 拿 uid（uid 不稳定，每次 click 前重新 snapshot）
click          uid=<...>
```

sidebar session button 现已带 `data-session-id` / `data-project-id`，定位优先 `[data-session-id="xxx"]` 而非 textContent 模糊匹配（textContent 在标题重复时会挑错）。

### 验证 checklist（每次 e2e 必跑）

| 项 | 工具 | 阈值 / 期望 |
|---|---|---|
| SSE OPEN 时间 | `browser-probes.js::T1 sseOpenLatency` | < 100ms（否则 prelude 没生效）|
| console error / warn | `list_console_messages types=["error","warn"]` | 0 error；warn 逐条判断 by-design vs bug |
| network over-fetch | `browser-probes.js::T4 networkOverFetch` 或 `list_network_requests` | 5s 内同 URL > 3 次 = silent over-fetch 嫌疑 |
| HTTP 4xx / 5xx | `list_network_requests` 扫 status | 0 个产品 4xx（自己测试代码的不算）|

**network over-fetch 是 e2e 才能抓的 silent perf bug**——sidebar 重复 fetch / file-change watcher 风暴 / SSE lagged 重拉。单测全绿但桌面端风扇起转都是这类。

### DOM selector / chunk 渲染验证

**先 take_snapshot 看真 class/attr 再写选择器**——Svelte 组件重命名后 selector 会变。

`browser-probes.js::T5 sessionDetailReady` 用 `.msg-row.msg-row-{user,ai}` 当前有效，但**任何时候 selector 不工作先 take_snapshot 看 DOM**，别盲改测试。

## 已踩坑（按主题）

### SSE / proxy

- **SSE 通过 vite proxy 卡 OPEN 3s+**：axum SSE 无 prelude 时，浏览器 EventSource 等首字节才进 OPEN，vite proxy 缓冲拖到 3s 超时。修法 `crates/cdt-api/src/http/sse.rs::sse_handler` —— `prelude` chain 一条 `Event::default().comment("open")`。回归判定：`T1 sseOpenLatency` 应 <100ms。

### URL 路径编码

- **group_id 含 `/` 必须 percent-encode**：路由 `{group_id}` 段必须传 `%2F`。`encodeURIComponent` 或 `python3 -c "import urllib.parse;print(urllib.parse.quote(x, safe=''))"`。SKILL `probe.sh` 已封装；shell 里直接 curl 时同理。

### 浏览器 transport 路由

- **加新 IPC command 必须同步 4 处**——漏一处 BrowserTransport throw `BrowserUnsupportedError`：
  1. `crates/cdt-api/src/http/routes.rs` 加 axum route
  2. `ui/src/lib/transport.ts::httpRequestForCommand` 加 case
  3. `LIST_SESSIONS_LIKE_COMMANDS`（如果该 cmd 触发后端 spawn 后台 emit metadata）
  4. `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` + handler case

  **判断 by-design 还是漏配**：grep `transport.ts::unsupportedBrowserCommands` Set——里面的是**故意不暴露**给浏览器（如 `read_agent_configs` / `check_for_update`），warn 是 by-design 不是 bug。

### chrome-devtools mcp

- **take_snapshot 后 uid 部分会变**（如 2_15 → 3_7）——uid 是 per-snapshot 编号，**click 前必须重新 take_snapshot**。
- **`reload` 不重置 BrowserTransport 单例**：浏览器 reload 重跑 main.ts 拿新 EventSource，但 `?http=1` URL 不变所以走真后端。验证 fresh state 用 `navigate_page type=reload ignoreCache=true`。
- **virtualized list `querySelectorAll` 只能拿可见范围**：sidebar vlist 只渲染 ~15-25 DOM 节点。要看"page N 是否对"先 scroll：`el.scrollTop = el.scrollHeight` + dispatch scroll event 后 take_snapshot；或用 `__cdtTest.openTab` 绕过。
- **Promise 超 ~5s 不 resolve 被回收**：`evaluate_script` 内 `setTimeout` 等几秒会报 `Protocol error: Promise was collected`。改 `wait_for` 或多次 evaluate + 中间 sleep。

### 数据缓存与扫描时序

- **list_group_sessions 第一次 cache miss 全骨架**：title 通过 SSE patch 异步上来。`sleep 5-10s` 等 scan 跑完再 snapshot；想可控复现用 `probe.sh --cache-miss`。
- **后台 scan 受 generation race 影响**：连续切 group / loadMore / silent refresh 会触发新 scan 抢占旧 scan。诊断：`probe.sh --cache-miss <group>` 跑两次，看第二次 title 是否 cache hit fill；fill 不全 = scan_key 设计有问题。

### 前端改动专属坑

- **mockIPC fixture ≠ 真后端**：fixture 数据是静态的 + 边界场景少（如 0 sessions / 长 title / 多 worktree），真后端跑出来才暴露列表渲染 / virtualization / IPC 字段名错配等问题。
- **`?http=1` 入口下没注入 `window.__cdtTest`（已修复）**：历史上 main.ts `if (params.has('http')) return` 把 helper 注入也跳过了——已把 `__cdtTest` 注入移到 return 之前。如未拉取此修法，e2e 只能靠 sidebar click + 模糊匹配。
- **session button 历史无 `data-session-id`（已修复）**：现已加 `data-session-id` + `data-project-id`，e2e 优先用 attr 而非 textContent 匹配。

## 时间预算（incremental，已 warm）

| 步骤 | 耗时 |
|---|---|
| start.sh 全流程（cdt-cli + vite + proxy check）| ~10s（实测）|
| cdt-cli 首次 cold build | 30-90s |
| chrome-devtools mcp navigate + hydrate | 3-5s |
| 后端 scan 100 条 metadata | 2-8s |
| 浏览器 SSE OPEN（含 `:open` prelude）| <100ms（4ms 实测）|
| list_group_sessions 5×P50 (238 sessions group) | ~180ms 实测 |

## 跟其他 skill 的边界

- **本 skill**：真后端 + 浏览器 + chrome-devtools mcp 端到端验证 fix 真生效
- `perf-bench`：纯 backend bench 跑分（cargo test --release），不开浏览器
- `wait-ci`：PR push 后等远端 CI 全绿，与本地浏览器验证互补
- `preflight`：开工前 fmt/lint/test 一把梭，不验证用户感知

修完行为类 bug 的标准链路：

```
cargo test / vitest（单元）→ 本 skill（curl + 浏览器 e2e + over-fetch 扫）→ push → wait-ci → codex
```

跳过本 skill 直接 push 的常见后果：unit test 全绿但桌面端仍报 bug——cursor / cache / SSE 时序 / sidebar 重复 fetch / IPC 字段名错配在单元测里没覆盖，curl + 浏览器才能抓。

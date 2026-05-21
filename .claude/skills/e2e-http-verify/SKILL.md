---
name: e2e-http-verify
description: 用 cdt-cli HTTP server + vite proxy + 浏览器 `?http=1` 入口跑端到端真数据验证 backend 修法。**只要**用户说"用 cli 起 server / 用 http 接口验证 / 浏览器自动化测试 / 真数据 e2e / chrome devtools 端到端 / 验证修法 / 用户感知 bug 复现 / 桌面端看不到修复 / sidebar 显示异常 / 切 group 慢"或显式 `/e2e-http-verify`，**都用这个 skill**——避免每次重新摸 vite proxy / SSE prelude / BrowserTransport 路由 / 端口冲突 / chrome-devtools mcp evaluate_script 陷阱，也避免修完只跑 unit test 就声称完成（用户的桌面端 binary 不一定用上新代码）。
---

# e2e-http-verify

修 backend / UI 行为后**先在浏览器里跑真数据看一遍**再宣告完成。Unit test + curl 验证 backend 数据形态，浏览器 e2e 验证 store → UI 渲染整条链路。

**只在 dev 调试场景用**（不影响 release bundle）。用户桌面 Tauri app 走 IPC 不走 HTTP server，本 skill 验证通过 ≠ 用户桌面端自动修好——结尾要明确告诉用户重启 cargo tauri dev。

## 何时触发

- 用户报 sidebar / 会话列表 / 切 group 切 worktree 类 UI bug，需要复现
- 修完 backend metadata / cursor / IPC 后想端到端验证
- chrome-devtools mcp 浏览器自动化跑真后端数据
- curl 直接看 list_group_sessions / list_sessions / SSE 推送内容

## 何时**不**触发

- 修 nothing 跑 nothing 只是"看看效果"——直接 take_snapshot 即可
- 改的是 Tauri 专属 API（通知 / 托盘 / setBadgeCount）——HTTP transport 不暴露这些
- 改的是只在 Tauri runtime 用的 chrome-devtools / SSE 不会经过的路径

## 流程（按顺序跑）

### 0. preflight 端口

```bash
lsof -iTCP:3456 -sTCP:LISTEN
lsof -iTCP:5173 -sTCP:LISTEN
```

- `:3456` 上有 `claude-de` / `claude-devtools-tauri` 进程 → 用户的桌面 app 占着，**不要 kill**——它跑的是旧版 binary，但你 cdt-cli 起不来。先告诉用户退出桌面 app，或者跑 cdt-cli 到别的端口（暂时改 config.http_server.port）。
- `:3456` 空 → 可以起 cdt-cli。
- `:5173` 空 → 可以起 vite。被旧 vite 占（典型 `pnpm.*vite` 或 `node.*vite`）→ `pkill -f vite` 后再起。

### 1. 起 cdt-cli HTTP server（`:3456`）

```bash
cargo run -p cdt-cli > /tmp/cdt-cli.log 2>&1 &
sleep 6
tail -3 /tmp/cdt-cli.log  # 应看到 "HTTP server listening on 127.0.0.1:3456"
```

第一次 build 慢，后续 incremental 几秒。`>/tmp/cdt-cli.log` 是必要的——否则 stdout 阻塞会让进程 hang。

### 2. 起 vite dev server（`:5173`）+ /api proxy

`ui/vite.config.ts` 已配 `server.proxy['/api']` → `http://127.0.0.1:3456`。直接：

```bash
pnpm --dir ui run dev > /tmp/vite-dev.log 2>&1 &
sleep 4
curl -s -o /dev/null -w 'proxy: %{http_code}\n' http://127.0.0.1:5173/api/repository-groups
# 期望 200
```

改 `vite.config.ts` 后**必须 `pkill -f vite` 再起**——vite config 不 hot reload。

### 3. curl 直接验证 backend 数据形态

不走 UI 也能验证 backend 修法正确性，最快路径。

```bash
GROUP_ID='/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.git'
ENC=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$GROUP_ID', safe=''))")

# 拉全部 sessions 看 metadata 完整性
curl -s "http://127.0.0.1:3456/api/repository-groups/$ENC/sessions?pageSize=500" > /tmp/page.json
python3 <<PY
import json
p = json.load(open('/tmp/page.json'))
total = len(p['sessions'])
titled = sum(1 for s in p['sessions'] if s.get('title'))
print(f"total={total} with_title={titled} missing={total-titled}")
# 看具体 sessionId
for s in p['sessions']:
    if s['sessionId'].startswith('7e61f52b'):
        print(f"  title={s.get('title')!r}")
        break
PY
```

第一次 cache miss 全 skeleton，title=null；等 5-10s 让 `scan_metadata_for_page` 跑完，第二次 curl 才看到 title fill。

### 4. chrome-devtools mcp 浏览器 e2e

URL：`http://127.0.0.1:5173/?http=1`。`?http=1` 在 `ui/src/main.ts` 跳过 setupMockIPC 让 BrowserTransport 接管。

```
mcp__chrome-devtools__navigate_page  url=http://127.0.0.1:5173/?http=1
sleep 4   # 等 UI hydrate + SSE OPEN
mcp__chrome-devtools__list_console_messages  types=["error","warn"]
mcp__chrome-devtools__take_snapshot
```

后续 click / scroll：

```
mcp__chrome-devtools__click  uid=<from-snapshot>
mcp__chrome-devtools__evaluate_script  function=...
```

### 5. 收尾 kill

```bash
pkill -f 'target/debug/cdt' 2>/dev/null
pkill -f 'pnpm.*vite\|node.*vite' 2>/dev/null
sleep 1
lsof -iTCP:3456 -sTCP:LISTEN  # 应空
lsof -iTCP:5173 -sTCP:LISTEN  # 应空
```

`run_in_background` 模式下也要显式 `pkill` —— `run_in_background` 进程不会随 session 结束自动死。

## 已踩坑速查（每条都被踩过 ≥1 次，2026-05-21）

### 端口与进程

1. **`:3456` 被用户桌面 app 占着**：`lsof` 看到 `claude-de` PID 是桌面 Tauri app（无论是否启 server-mode 都占 3456 作 backend port）。不要 kill 用户进程；要么让用户退出 app，要么 cdt-cli 改端口。
2. **`run_in_background` 残留**：上轮 `cargo run -p cdt-cli` 即使 task 状态显示 `failed`，子进程可能还活着 LISTEN。用 `lsof` 确认 + `pkill -9` 兜底。
3. **stdout 阻塞 cdt-cli hang**：必须 `> /tmp/cdt-cli.log 2>&1`，不能直接 `cargo run -p cdt-cli &` 让 stdout 写 terminal。

### vite proxy

4. **vite.config.ts 改完不重启不生效**：`server.proxy` 改了必须 `pkill -f vite` 重启。Svelte HMR 不覆盖 dev server config。
5. **SSE 通过 vite proxy 卡 OPEN 3s+**：默认 axum SSE 无 prelude，浏览器 EventSource 等首字节才进入 OPEN，vite proxy 缓冲可能拖到 3s 超时。修法在 `crates/cdt-api/src/http/sse.rs::sse_handler` —— `prelude` chain 一条 `Event::default().comment("open")`。验证：浏览器 evaluate_script `new EventSource('/api/events')` onopen 应 <100ms。

### URL 路径编码

6. **group_id 含 `/` 必须 percent-encode**：`/Users/.../claude-devtools-rs/.git` 含 6 个 `/`，全部要 → `%2F`。直接 curl `?cursor=...` 时同理（base64 含 `=`、`+` 也要 encode）。`encodeURIComponent` 即可，shell 里 `python3 -c "import urllib.parse;print(urllib.parse.quote(x, safe=''))"`。
7. **axum 0.8 默认接受 percent-encoded `/`**：路由 `/api/repository-groups/{group_id}/sessions` 能匹配 `%2F` 解码后含 `/` 的 group_id。验证：`curl` 200 OK = 路由 OK。

### 浏览器 transport 路由

8. **加新 IPC command 必须同步 4 处**：`crates/cdt-api/src/http/routes.rs` 加 axum route + `ui/src/lib/transport.ts::httpRequestForCommand` 加 case + `LIST_SESSIONS_LIKE_COMMANDS` 集合（如果该 cmd 触发后端 spawn 后台 emit metadata）+ `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 列表 + handler case。漏一个 BrowserTransport throw `BrowserUnsupportedError`。

### chrome-devtools mcp evaluate_script

9. **Promise 超时报 `Protocol error: Promise was collected`**：evaluate_script 内 Promise 超过 ~5s 不 resolve 会被回收。拆成多次调用 + 中间 sleep，不要在一个 evaluate 内 `setTimeout` 等几秒。
10. **`reload` 不重置 BrowserTransport 单例**：浏览器 reload 重新跑 main.ts → 重新 setup → BrowserTransport 是模块级单例 → reload 后是**新的** EventSource。但 `?http=1` URL 不变所以走真后端。验证 fresh state 用 `navigate_page type=reload ignoreCache=true`。
11. **virtualized list `querySelectorAll('.session-item')` 只能拿到 visible 范围**：sidebar vlist 只渲染 ~15-25 DOM 节点不管总 session 数。要看"page N 是否 title 正确"必须 `scrollTop = scrollHeight` + dispatch scroll event 后 take_snapshot。

### 数据缓存与扫描时序

12. **list_group_sessions 第一次 cache miss 全 skeleton**：title 通过 SSE patch 异步上来，浏览器侧依赖 EventSource 真正 OPEN。`sleep 5-10` 等 scan 跑完再 take_snapshot，否则会看到一堆 sessionId fallback 误判 bug。
13. **后台 scan 受 generation race 影响**：连续切 group / loadMore / silent refresh 会触发新 scan 抢占旧 scan。如果 scan_key 设计不当（hash 碰撞 / 单 namespace），旧 scan 被 abort → 那部分 metadata 永久丢失。诊断：curl 直接拉同一 cursor 第二次，看 title 是否 cache hit fill。

## 经验性时间预算

| 步骤 | 耗时 |
|---|---|
| cdt-cli 冷启 build + start | 5-30s（首次）/ 5-8s（incremental） |
| vite 起 + warmup | 2-4s |
| chrome-devtools mcp navigate + hydrate | 3-5s |
| 后端 scan 跑完 100 个 jsonl metadata | 2-8s（取决于 jsonl 大小、并发限流 8）|
| 浏览器 SSE OPEN（含 `:open` prelude）| <50ms |
| 浏览器 SSE OPEN（无 prelude）| 3000ms+（vite proxy 缓冲） |

## 跟其他 skill 的边界

- **本 skill** 专做"真后端 + 浏览器 + chrome-devtools mcp 端到端验证"——验证 fix 真生效，不是验证算法正确
- `perf-bench`：纯 backend bench 跑分（cargo test --release），不开浏览器
- `wait-ci`：PR push 后等远端 CI 全绿，与本地浏览器验证互补
- `preflight`：开工前 fmt/lint/test 一把梭，不验证用户感知

修完 backend bug，理想的验证链路是：

```
cargo test (单元) → 本 skill curl + chrome-devtools mcp (集成) → push (远端 CI) → wait-ci
```

跳过本 skill 直接 push 的常见后果：**unit test 全绿但用户桌面端仍报 bug**——典型是 cursor / cache / SSE 时序问题在单元测里没覆盖，curl + 浏览器才能复现。

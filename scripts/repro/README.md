# scripts/repro/ SSH 类问题诊断脚本

本目录是 **SSH 类 bug 的本地诊断工具集**——不是 CI 测试，不是产品代码。每个脚本针对一个具体的复现场景，配 docker 容器 + 本地 SSH 公钥跑，跑完留下完整时序日志方便人工解读。

## 什么时候用这些脚本

按场景对号入座，找不到对应脚本说明你遇到的问题不在已知场景里——可以参考现有脚本风格新写一个。

### 场景 1：本地和 docker 端用同一份数据，分不清当前看到的是 SSH 还是 Local

**症状**：
- 在桌面端切到 SSH context，sidebar 里看到的项目名跟你本地 `~/.claude/projects/` 一模一样
- 切回 Local 后看到一样的项目名，无法判断切换是否生效
- 怀疑某个 IPC 调用走错路径（SSH vs Local），但内容一样没法证伪

**根因**：`cdt-ssh-test` docker 容器是 `ro` bind-mount 你本地 `/Users/.../.claude` —— **物理上**两边数据就是同一份。

**用**：[`setup-ssh-fixture-container.sh`](#setup-ssh-fixture-containersh)（独立容器 + 独立 fixture）

---

### 场景 2：怀疑 SSH 长连接断开 / SFTP 异常 / 网络抖动后，应用卡在 SSH 状态不切回 Local

**症状**：
- 远端 sshd 死了 / 网络断了 / VPN 掉了 / 笔记本休眠唤醒后，应用看上去"还在 SSH"
- polling watcher 应该 N 秒内识别死亡触发自愈 disconnect，但实际没触发
- 切回 Local 后会话列表加载慢

**复现工具**：用 `pkill -STOP sshd` 暂停容器内 sshd 进程，制造一个**真实** SFTP 死 channel 场景（对端不响应而非 close）。这比"网络断"更精确，能稳定复现 hang 类问题。

**用**：[`repro-ssh-dead-channel.sh`](#repro-ssh-dead-channelsh)

> 当前跑这个脚本会**复现失败**——polling watcher 60s 内不会切回 Local。这正是 `openspec/followups.md::ssh-remote-context` 记录的 SFTP timeout 不识别 bug。修复 PR 落地后跑这个脚本应该在 ~9s 内切回 Local，作为回归 fixture。

---

### 场景 3：改了 ssh_disconnect / context_changed / cancel_remote_watcher 等代码，担心引入性能回归

**症状**：
- 你的 PR 改了 `LocalDataApi::ssh_disconnect`、`ssh_mgr.disconnect`、`abort_scans_for_ssh_context_id`、`cancel_remote_watcher` 任一处
- 想知道改完后 `ssh_disconnect` IPC 本身耗时、切回 Local 后立刻调 list 是否变慢

**用**：[`repro-disconnect-perf.sh`](#repro-disconnect-perfsh)

---

### 场景 4：怀疑大量 project / session 数据下 SSH 路径性能下降

**症状**：
- 默认 fixture 太干净（3 项目 × 1 session × 4 消息）跑得太快，不能反映真实数据规模
- 怀疑 list_repository_groups / list_sessions 在数据规模上去后慢

**用**：[`setup-ssh-fixture-container.sh`](#setup-ssh-fixture-containersh) 加 `--scale N M K` 参数生成大规模合成数据。

---

## 一次性准备

```bash
# 预编译 cdt-cli，让脚本里的 cargo run 秒起（首次会下 toolchain + 编译，之后秒起）
cargo build -p cdt-cli --bin cdt
```

确保你有 `~/.ssh/id_rsa.pub` 或 `id_ed25519.pub`（脚本会自动找并塞进容器作为 SSH 公钥）。

---

## 脚本详解

### setup-ssh-fixture-container.sh

启动**独立** SSH 测试容器（容器名 `cdt-ssh-fixture-test`，host port 2223），mount 一个**独立的 RW fixture home** 到 `/config/.claude`——**不复用**你本地 `~/.claude`。和现有 `cdt-ssh-test` 容器（port 2222，ro mount 你本地）共存，互不影响。

注入的 fixture 用 `/srv/ssh-fixture-NNN` 路径（编码后 project_id 是 `-srv-ssh-fixture-NNN`）——这种 ID 你本地 `~/.claude/projects/` 绝不会有。看到这种 ID 在 sidebar = SSH 数据真的加载出来了；看不到 = 当前在 Local 或 SSH 路径没走通。

```bash
# 默认 3 项目 × 1 session × 4 消息（小，验证基本切换）
bash scripts/repro/setup-ssh-fixture-container.sh up

# 大规模（接近真实数据，复现规模相关的性能问题）
bash scripts/repro/setup-ssh-fixture-container.sh up --scale 50 10 20

# 仅刷新 fixture，容器不动（用于改了 fixture 内容快速 reload）
bash scripts/repro/setup-ssh-fixture-container.sh refresh --scale 100 5 10

# 看容器和 fixture 状态
bash scripts/repro/setup-ssh-fixture-container.sh status

# 完全清理（停 + 删容器 + 删 fixture home）
bash scripts/repro/setup-ssh-fixture-container.sh down
```

`up` 完成后用如下配置在桌面端添加新 SSH 连接：

| 字段 | 值 |
|---|---|
| Host | `localhost` |
| Port | `2223`（覆盖：`CDT_SSH_FIXTURE_PORT`） |
| Username | `devuser` |
| Auth | SSH config（自动用你 `~/.ssh/id_*.pub`） |

---

### repro-ssh-dead-channel.sh

用 `docker exec cdt-ssh-test pkill -STOP sshd` 暂停容器内 sshd 进程，制造 SFTP 死 channel。然后立刻调 `list_sessions` + 监控 active context 切换时序，看 polling watcher 多久识别 dead 并触发 self-heal。

完成后 trap 自动 `pkill -CONT sshd` 恢复。

```bash
bash scripts/repro/repro-ssh-dead-channel.sh
```

**前置条件**：`cdt-ssh-test` 容器运行在 port 2222（即 `scripts/verify-ssh-docker-e2e.sh` 配套的容器，**不是** fixture 容器）。

---

### repro-disconnect-perf.sh

测量 `ssh_disconnect` IPC 自身耗时 + 切回 Local 后多次 `list_repository_groups` / `list_sessions` 调用耗时。当前基线（v0.5.3 · 2026-05-22）：

| 阶段 | 耗时 |
|---|---|
| `ssh_disconnect` IPC | ~165 ms |
| post-disconnect `list_repository_groups` × 3 | ~67 ms / ~64 ms / ~64 ms |

跑你的 PR 改动后对比：任一项显著变差就值得查 regression。

```bash
bash scripts/repro/repro-disconnect-perf.sh
```

**前置条件**：同上，`cdt-ssh-test` 容器（port 2222）。

---

## 为什么 CI 不跑这些脚本

CI runner 没有：
- 长期运行的 docker 容器
- 你本地的 `~/.ssh/id_*.pub`
- localhost:2222 / :2223 的稳定可用环境

这些脚本是**人工诊断工具**——出 bug 时用来稳定复现 + 对比基线，跑完看日志解读。本地跑一次的成本远低于反复人工试错。

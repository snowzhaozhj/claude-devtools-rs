# ssh-remote-context Specification

## Purpose

定义"上下文"抽象（本地 / SSH 远程）以及 SSH 连接的建立、状态查询与拆除规则，使下游 capability（`project-discovery`、`session-parsing`、`session-search`）能够以统一接口同时消费本地和远端的 Claude 会话数据。

## Requirements

### Requirement: Manage local and SSH contexts

系统 SHALL 暴露"上下文"概念，表示会话数据的来源，分两类：`local`（宿主机文件系统）与 `ssh`（远程主机）。系统 SHALL 提供列出上下文、切换当前上下文、查询当前激活上下文的能力。

#### Scenario: Default local context
- **WHEN** 应用启动且无既有 SSH 状态
- **THEN** 当前上下文 SHALL 为 `local`，绑定本地文件系统 provider

#### Scenario: Switch to SSH context
- **WHEN** 调用方请求切换到一个已建立的 SSH 上下文
- **THEN** 后续 session discovery 与读取 SHALL 走 SSH 文件系统 provider

### Requirement: Establish and tear down SSH connections

系统 SHALL 通过 SSH 连接到远程主机，连接时 SHALL 在 `~/.ssh/config` 存在的情况下读取主机元数据；SHALL 支持显式断开与应用退出时的优雅断开。

#### Scenario: Connect by host alias from ssh config
- **WHEN** 调用方请求连接到 `~/.ssh/config` 中已定义的 alias
- **THEN** 系统 SHALL 从 ssh config 解析出 hostname、user、port、identity file 并建立连接

#### Scenario: Test connection without persisting
- **WHEN** 调用方请求测试连通性
- **THEN** 系统 SHALL 尝试鉴权并返回成功或错误详情，且 SHALL NOT 把该连接登记为激活上下文

#### Scenario: Disconnect
- **WHEN** 调用方断开一个已激活的 SSH 上下文
- **THEN** 连接 SHALL 被关闭，后续从该上下文的读取 SHALL 以明确的错误失败

### Requirement: Read sessions and files over SSH with same contract

系统 SHALL 在 SSH 上下文上提供与 local 上下文等价的 project-discovery、session-parsing、文件读取能力，使下游消费者观察到完全相同的数据形状。

#### Scenario: List projects on a remote host
- **WHEN** 当前上下文是 SSH，调用方请求项目列表
- **THEN** 返回结果 SHALL 与本地项目列表形状一致，数据源为远程 `~/.claude/projects/` 目录

#### Scenario: Read a remote session
- **WHEN** 当前上下文是 SSH，调用方请求会话详情
- **THEN** 系统 SHALL 流式读取远程 JSONL 文件并返回与本地输出形状一致的 chunk 序列

### Requirement: Report SSH connection status

系统 SHALL 暴露每个已配置 SSH 上下文的连接状态（`disconnected` / `connecting` / `connected` / `error`），错误状态 SHALL 附带可读的错误说明。

#### Scenario: Query status of a failed context
- **WHEN** 某个 SSH 上下文连接失败
- **THEN** 状态查询 SHALL 返回 `error` 与底层错误信息

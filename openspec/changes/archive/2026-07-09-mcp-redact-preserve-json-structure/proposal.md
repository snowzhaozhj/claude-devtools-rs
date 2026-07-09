## Why

MCP secret redaction 当前对**序列化后的紧凑 JSON 字符串**做正则替换。`password` 规则 `(?i)password\s*[=:]\s*\S+` 含无界 `\S+`，紧凑 JSON 无结构空格时会从 `password=` 起贪婪吃穿后续字段与结构字符，`[REDACTED]` 替换后 JSON 残缺 → `emit_json` 的 `serde_json::from_str` 失败 → 静默回退成被截断的字符串。MCP 客户端拿到腰斩的 blob，且无从得知丢了多少数据。默认脱敏是开的，任何含 `password=`/`password:` 的 session（env、连接串、配置极常见）都可能中招——这是默认配置下的静默数据丢失（GitHub #596，来自 `/bug-hunt`）。

## What Changes

- 脱敏从「序列化后对 JSON 字符串跑正则」改为「对 `serde_json::Value` 递归脱敏」：对字符串叶子值与对象 key 跑 secret 正则，结构字符（`{}[]",:`）永不进替换目标，从根因上消除脱敏破坏 JSON 结构的可能。
- `password` 正则保持原 `\S+`（apply 阶段撤销了一度收窄为 `[^\s"]+` 的想法——见 design D2b：收窄会在密码值含引号时欠脱敏，而 D1 已保证结构安全）。
- 输出契约不变：仍是 `{data, redacted: true, redactedCount: N}` 包裹，字段名与 `--allow-sensitive` 语义不动。

## Capabilities

### Modified Capabilities
- `mcp-server`: `Requirement: Secret redaction` 新增 scenario——脱敏 SHALL 不破坏响应 JSON 结构（脱敏后返回体始终是合法 JSON，未命中字段完整保留）。

## Impact

- 代码：`crates/cdt-cli/src/mcp/redact.rs`（新增递归 Value 脱敏）、`crates/cdt-cli/src/mcp/mod.rs::emit_json`。
- 输出契约：无字段名 / 结构变化（只修复"命中时响应被腰斩"的缺陷）。
- 测试：`cargo test -p cdt-cli`。
- 追踪：GitHub #596。

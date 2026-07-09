## Context

MCP server 默认对所有 tool 返回做 secret 脱敏（`--allow-sensitive` 跳过）。当前实现（`crates/cdt-cli/src/mcp/redact.rs` + `mcp/mod.rs::emit_json`）流程：

1. `serde_json::to_string(value)` 把响应序列化成**紧凑 JSON 字符串**（无结构空格）。
2. 对整个字符串逐条跑 secret 正则做 `replace_all`。
3. 若命中 > 0，`serde_json::from_str` 把脱敏后字符串重新解析成 `Value` 包进 `{data, redacted, redactedCount}`；解析失败则 `unwrap_or` 回退成 `Value::String(text)`。

缺陷（GitHub #596）：`password` 规则 `(?i)password\s*[=:]\s*\S+` 的 `\S+` 无界。紧凑 JSON 无空格，`\S+` 从 `password=` 起吃到下一个**真实空白**——往往是若干字段之后，甚至整段结尾。结果 `[REDACTED]` 把中间的 `","field":"..."` 等结构字符一并吞掉 → JSON 残缺 → step 3 的 `from_str` 失败 → 静默回退成被腰斩的字符串。客户端丢失从首个匹配起的全部数据且无感知。

根因是「在序列化文本层做替换」——正则无法感知 JSON 结构边界。

## Goals / Non-Goals

**Goals:**
- 脱敏后返回体**始终是合法 JSON**，未命中的字段完整保留，只有命中的 secret 子串被替换为 `[REDACTED]`。
- 保持输出契约不变：`{data, redacted: true, redactedCount: N}` 包裹、字段名、`--allow-sensitive` 语义、`[REDACTED]` 占位符均不动。
- 保持既有 secret pattern 覆盖（Anthropic `sk-`、AWS `AKIA`、GitHub `ghp_/gho_`、Bearer、password、PEM、JWT）。

**Non-Goals:**
- 不改 secret pattern 的检测种类（除 D2 收窄 password 边界外）。
- 不改 CLI 侧输出、不新增 IPC 字段。
- 不覆盖出现在 JSON 值/ key 中的**非** secret-pattern 数据（脱敏只按既有 secret 正则命中，不做语义判断）。

## Decisions

### D1：脱敏作用于 `serde_json::Value` 的字符串（叶子值 + 对象 key），而非序列化文本

`Redactor` 新增对结构化 `Value` 的递归脱敏：遍历 `Value`——对 `Value::String` 叶子跑现有 secret 正则替换；`Object` 对**每个 key 字符串也跑替换**、并递归其 value；`Array` 递归每个元素；数字/布尔/null 原样。累加所有命中数。

**为什么连 key 一起脱**（codex design 二审 finding）：`get_tool_output` 返回的 `ToolOutputView::Structured { value: serde_json::Value }` 原样保留原始工具调用的**任意 JSON 对象**——其对象 key 可能含用户数据（含 secret），并非全是固定 schema 字段名。旧的「序列化后文本正则」实现连 key 带 value 一起替换；若 D1 只脱字符串值、放过 key，会相对旧行为**回退** structured output 里 key 内 secret 的覆盖。故递归时对 key 字符串同样跑替换。

`emit_json` 改为：`serde_json::to_value(value)` → 递归脱敏得到 `(redacted_value, count)` → `count > 0` 时包 `{data: redacted_value, redacted: true, redactedCount: count}`，否则原样 emit `redacted_value`（等价原值）。

**理由**：替换只发生在字符串叶子内部，结构字符（`{}`、`[]`、`"`、`,`、`:`）永远不在替换目标里，从根因上杜绝 JSON 破坏。不再需要「序列化 → 正则 → 反序列化 → 失败回退」这条脆弱链路，`from_str` 回退分支随之删除。`redactedCount` 语义从「文本层匹配段数」变为「所有字符串叶子内的匹配总数」——对既有 scenario（单 key 命中）结果一致。

### D2：`password` 正则无界 `\S+` 收窄为 `[^\s"]+`（纵深防御）

即便某个字符串叶子的**值内部**含 `password=secret" ...`（叶子内嵌引号，罕见），收窄后也不会把叶子内引号之后的内容吞进匹配。正确性主要由 D1 的「只作用于字符串叶子」保证，D2 只是把单叶子内的过度匹配面进一步缩小。其余 7 条规则的字符类已隐式排除结构字符，无需改。

### D3：保持包裹结构与字段名不变

`data` / `redacted` / `redactedCount` 字段名、`--allow-sensitive` 关闭脱敏、未命中不加 wrapper——全部不变。本 change 是纯缺陷修复，不动输出契约。

## Risks / Trade-offs

- **两个不同的 secret 若恰好构成完整 key 且脱敏后同形，会在对象里塌成一个 key**：如 `"AKIA....A"` 与 `"AKIA....B"` 两个 key 整体都是 secret-pattern → 都脱成 `"[REDACTED]"` → serde_json Map 后写覆盖丢一个。属病态输入（secret 作完整对象 key），且脱敏本就是有损操作，可接受。key 内非整体命中（如 `"password=x"` 作 key）只替换命中子串，不塌陷。
- **`redactedCount` 计数口径变化**：从「文本正则匹配段数」变为「字符串叶子内匹配总数」。既有两个 scenario 只断言 `redacted: true` 与替换结果，不断言精确 count，故不回归；新测试按叶子匹配口径断言。
- **性能**：递归遍历 `Value` 而非单次字符串扫描，量级相同（都要遍历全部内容），无热路径回归风险；MCP 响应本就已在内存中构造。

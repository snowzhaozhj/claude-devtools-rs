# 通用 code smells catalog

任何 scope 都先过一遍这 10 项。命中后的处理路径走 SKILL.md::§2 4 路分流。

## 命名 + 一句话定义

| category | 一句话定义 | 典型修法 |
|---|---|---|
| **god-function** | 函数 > 50 行做 ≥ 3 件事，命名很难概括 | extract method，按职责拆子函数 |
| **god-class / god-module** | 类 / 模块 > 500 行 / > 10 公开方法 / 多个职责混杂 | 按职责 split 模块，trait / impl 分组 |
| **duplicated-code** | 同样逻辑 ≥ 2 处（不仅文本，语义相同也算） | extract 共享函数 / trait method / 配置数据 |
| **long-param-list** | 函数参数 > 4 个，调用方记不住顺序 | introduce param object / builder |
| **magic-number / magic-string** | 数值 / 字符串字面量未命名，意图不显 | 提常量 + 注释（写"为什么是这个值"，不是"这是 X"） |
| **nested-conditionals** | if 嵌套 ≥ 4 层 / arrow code | guard clauses 早返回 / Result 链式 / `let-else` |
| **dead-code** | 未引用的函数 / 字段 / import / 注释代码 / `#[allow(dead_code)]` 长期挂着 | 删（git 有历史，需要时找回来） |
| **feature-envy** | 方法读另一个对象的字段 > 自己的字段 | 把方法移到数据所在的类型 |
| **primitive-obsession** | 用 `String` / `u64` / `Vec<...>` 表达领域概念，校验逻辑散落 | newtype / enum / domain struct |
| **inappropriate-intimacy** | 一个模块深入另一个模块的私有实现（链式 `.field.field.field`） | "ask, don't tell"——加方法封装内部访问 |

## 量化阈值（仓内默认）

- 函数：> 50 行 → medium；> 100 行 → high
- 模块 / 文件：> 500 行 → medium；> 1000 行 → high（需评估是否拆 crate）
- 嵌套：> 3 层 → medium；> 5 层 → high
- 参数：> 4 → medium；> 6 → high
- 重复块：≥ 2 处 5+ 行相同 → medium；≥ 3 处 → high

阈值是启发式，不是硬约束——具体看可读性 / 维护成本，例如 100 行的纯数据匹配 `match` 不算 god-function。

## 常见 false positive

- **测试文件 `tests/` / `mod tests {}`**：fixture 重复、长 setup 不算 duplicated-code
- **生成代码 `*.generated.rs` / `target/`**：跳过
- **migration / schema 数据**：长 enum 列表合理，不算 god-class
- **trait impl 块**：实现方法多但每个都很短不算 god-class

## 输出 finding 时

`category` 字段直接用上表英文 key（如 `god-function`、`magic-number`），便于跨次 audit 跑 grep / diff 对齐。

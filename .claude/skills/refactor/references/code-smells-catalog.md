# 通用 code smells catalog

任何 scope 都先过一遍这 10 项。命中后按 SKILL.md §5 输出格式记到 findings 表（结构反模式打 category 名；若同时命中 SKILL.md §2 boundary guard 5 类则改打 `boundary-<n>-<short>`）。

## 命名 + 一句话定义

| category | 一句话定义 | 典型修法 |
|---|---|---|
| **god-function** | 函数 > 80 行做 ≥ 3 件事，命名很难概括 | extract method，按职责拆子函数 |
| **god-class / god-module** | 类 / 模块 > 800 行 / > 10 公开方法 / 多个职责混杂 | 按职责 split 模块，trait / impl 分组 |
| **duplicated-code** | 同样逻辑 ≥ 2 处（不仅文本，语义相同也算） | extract 共享函数 / trait method / 配置数据 |
| **long-param-list** | 函数参数 > 5 个，调用方记不住顺序 | introduce param object / builder |
| **magic-number / magic-string** | 数值 / 字符串字面量未命名，意图不显 | 提常量 + 注释（写"为什么是这个值"，不是"这是 X"） |
| **nested-conditionals** | if 嵌套 ≥ 5 层 / arrow code | guard clauses 早返回 / Result 链式 / `let-else` |
| **dead-code** | 未引用的函数 / 字段 / import / 注释代码 / `#[allow(dead_code)]` 长期挂着 | 删（git 有历史，需要时找回来） |
| **feature-envy** | 方法读另一个对象的字段 > 自己的字段 | 把方法移到数据所在的类型 |
| **primitive-obsession** | 用 `String` / `u64` / `Vec<...>` 表达领域概念，校验逻辑散落 | newtype / enum / domain struct |
| **inappropriate-intimacy** | 一个模块深入另一个模块的私有实现（链式 `.field.field.field`） | "ask, don't tell"——加方法封装内部访问 |

## 量化阈值（仓内默认 — Rust 友好版）

阈值偏宽，宁少报勿误报。Rust 风格里多 impl block / 长 match arms / inline `mod tests` 普遍，旧版（50 行 / 500 行 / 3 层）会让审计噪音过高。

- 函数：> 80 行 → medium；> 150 行 → high
- 模块 / 文件：> 800 行 → medium；> 1500 行 → high（需评估是否拆 crate）
- 嵌套：> 4 层 → medium；> 6 层 → high
- 参数：> 5 → medium；> 7 → high
- 重复块：≥ 2 处 8+ 行相同（且非 trait impl 模板）→ medium；≥ 3 处 → high

阈值是启发式，不是硬约束——具体看可读性 / 维护成本，并参考下面豁免清单。

## 豁免清单（命中以下情况**不**报）

**默认豁免**：
- 测试文件 `tests/` / `mod tests {}` / `*_test.rs` / `*.test.ts` —— fixture 重复 / 长 setup / 长 assertion 不算 duplicated-code
- 生成代码 `*.generated.rs` / `target/` / `node_modules/` / `.svelte-kit/` —— 跳过
- proc macro 展开后的代码（`expand` 输出） —— 跳过
- migration / schema 数据 / 长 enum / 大 const table —— 不算 god-class / god-function
- 每个方法都很短（< 15 行）的 trait impl 块 —— 总行数大不算 god-class

**Rust 特有豁免**：
- 同文件多 `impl Foo` / `impl<T> Foo for T` 块叠加（典型 builder / trait impls 同存）—— 总行数大不算 god-module
- 长 `match` 分发（每个 arm < 10 行，arm 数 > 10）—— 不算 god-function 也不算 nested-conditionals
- `Result` / `Option` 链式 `?` 解析（看着嵌套但实际是早返回） —— 不算 nested-conditionals
- `match` guard 内 `if let` 一两层 —— 不算 nested
- contract test / `#[serde]` 大量字段定义 —— 不算 god-function

**Svelte / TS 特有豁免**：
- `{#each}` / `{#if}` 模板嵌套 ≤ 3 层 —— 不算 nested-conditionals（模板层级是表达力的一部分）
- `<style>` 块大 —— 不算 god-module（CSS 不参与 LOC 阈值）

报 finding 前 SHALL 过一遍豁免清单——命中即跳过，不报到 audit。

## 输出 finding 时

`category` 字段直接用上表英文 key（如 `god-function`、`magic-number`），便于跨次 audit 跑 grep / diff 对齐。

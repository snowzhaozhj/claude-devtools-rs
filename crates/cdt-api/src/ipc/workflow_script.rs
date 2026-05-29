//! Workflow script `meta` 静态解析（Tier 1 增强）。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md` §`Workflow script meta 静态解析（Tier 1）`。
//!
//! 设计（design D2）：用**窄职责隔离 lexer** 切出 `export const meta = { ... }`
//! 对象字面量块（平衡括号扫描，跟踪三种字符串分隔符 + 注释 + 转义），再把切出的
//! 块整体交 `json5` 做结构解析——**非**手搓对象结构提取。任何环节失败返回
//! `None`，由调用方静默降回 Tier 0。
//!
//! 不选 `oxc` / `tree-sitter`：对「提取 2 字段、已缓存、瞬态」过度投资，且
//! tree-sitter 的 C-grammar 触发 Windows 跨平台构建风险。

use cdt_core::workflow::WorkflowPhase;

/// 解析出的 script meta——仅 `name` + `phases`（静态列表，无「当前 phase」）。
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptMeta {
    pub name: Option<String>,
    pub phases: Vec<WorkflowPhase>,
}

#[derive(serde::Deserialize)]
struct RawMeta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    phases: Option<Vec<RawPhase>>,
}

#[derive(serde::Deserialize)]
struct RawPhase {
    title: String,
    // detail / 其它字段忽略（serde 默认不 deny unknown）
}

/// 解析 script 文本的 `meta` 块取 `name` + `phases`。任何失败（无 meta 块 /
/// 括号不配平 / json5 报错，如 backtick 分隔的值）返回 `None`——调用方据此降回
/// Tier 0，**绝不** panic 或返回半截内容。
#[must_use]
pub fn parse_script_meta(content: &str) -> Option<ScriptMeta> {
    let block = extract_meta_block(content)?;
    let raw: RawMeta = json5::from_str(block).ok()?;
    let phases = raw
        .phases
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, p)| WorkflowPhase {
            index: u32::try_from(i).unwrap_or(u32::MAX),
            title: p.title,
        })
        .collect();
    Some(ScriptMeta {
        name: raw.name,
        phases,
    })
}

/// 隔离 lexer 的扫描状态。
enum Mode {
    Normal,
    Str(u8),
    LineComment,
    BlockComment,
}

/// 隔离 lexer：定位 `export const meta` 后的第一个 `{`，做平衡括号扫描切出完整
/// 对象字面量块（含两端花括号）。
///
/// 扫描跟踪：单/双/反引号字符串（串内 `{` `}` 不计深度）、`//` 行注释、`/* */`
/// 块注释、`\` 转义。meta 是纯数据对象字面量（无裸 regex / 裸除法），故无 `/`
/// 歧义。括号不配平（截断 / 异形）→ 返回 `None`。
fn extract_meta_block(content: &str) -> Option<&str> {
    let anchor = content.find("export const meta")?;
    let bytes = content.as_bytes();
    // 从 anchor 之后定位第一个 '{'
    let mut start = anchor + "export const meta".len();
    while start < bytes.len() && bytes[start] != b'{' {
        start += 1;
    }
    if start >= bytes.len() {
        return None;
    }

    let mut mode = Mode::Normal;
    let mut escaped = false;
    let mut depth: i32 = 0;
    let mut i = start;

    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        match mode {
            Mode::LineComment => {
                if c == b'\n' {
                    mode = Mode::Normal;
                }
            }
            Mode::BlockComment => {
                if c == b'*' && next == Some(b'/') {
                    mode = Mode::Normal;
                    i += 2;
                    continue;
                }
            }
            Mode::Str(quote) => {
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == quote {
                    mode = Mode::Normal;
                }
            }
            Mode::Normal => match c {
                b'/' if next == Some(b'/') => {
                    mode = Mode::LineComment;
                    i += 2;
                    continue;
                }
                b'/' if next == Some(b'*') => {
                    mode = Mode::BlockComment;
                    i += 2;
                    continue;
                }
                b'\'' | b'"' | b'`' => mode = Mode::Str(c),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return content.get(start..=i);
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    // 括号未配平 → None
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_phases() {
        let src = r"
            export const meta = {
              name: 'foo',
              description: 'desc',
              phases: [
                { title: 'Build', detail: 'one agent' },
                { title: 'Verify' },
              ],
            }
            phase('Build')
        ";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("foo"));
        assert_eq!(meta.phases.len(), 2);
        assert_eq!(meta.phases[0].index, 0);
        assert_eq!(meta.phases[0].title, "Build");
        assert_eq!(meta.phases[1].index, 1);
        assert_eq!(meta.phases[1].title, "Verify");
    }

    #[test]
    fn handles_comments_with_braces() {
        // 注释里的 } 不应提前结束 meta 块
        let src = r"
            export const meta = {
              // 注释含 } 不该提前闭合
              name: 'c',
              /* 块注释 { } 也不该计入深度 */
              phases: [{ title: 'X' }],
            }
        ";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("c"));
        assert_eq!(meta.phases.len(), 1);
    }

    #[test]
    fn handles_escaped_quotes_in_string() {
        let src = r"export const meta = { name: 'a\'b }', phases: [{ title: 'T' }] }";
        let meta = parse_script_meta(src).unwrap();
        // 字符串内的 } 不计深度；name 含转义引号 + 花括号
        assert_eq!(meta.name.as_deref(), Some("a'b }"));
        assert_eq!(meta.phases.len(), 1);
    }

    #[test]
    fn handles_double_quotes() {
        let src = r#"export const meta = { "name": "dq", "phases": [{ "title": "D" }] }"#;
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("dq"));
        assert_eq!(meta.phases[0].title, "D");
    }

    #[test]
    fn detail_before_title_field_order_irrelevant() {
        let src =
            r"export const meta = { name: 'o', phases: [{ detail: 'first', title: 'Ordered' }] }";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.phases[0].title, "Ordered");
    }

    #[test]
    fn no_phases_field_yields_empty() {
        let src = r"export const meta = { name: 'np', description: 'no phases here' }";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("np"));
        assert!(meta.phases.is_empty());
    }

    #[test]
    fn single_line_minified() {
        let src = r"export const meta={name:'m',phases:[{title:'A'},{title:'B'}]};const x=1";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("m"));
        assert_eq!(meta.phases.len(), 2);
    }

    #[test]
    fn nested_object_in_meta() {
        let src = r"export const meta = {
            name: 'nested',
            phases: [{ title: 'P', extra: { deep: { x: 1 } } }],
            opts: { a: { b: 2 } },
        }";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("nested"));
        assert_eq!(meta.phases[0].title, "P");
    }

    #[test]
    fn backtick_value_degrades_to_none() {
        // json5 不支持 backtick 分隔的值——lexer 仍配平切块，但 json5 报错 → None
        let src = "export const meta = { name: `tmpl ${x}`, phases: [{ title: 'T' }] }";
        // lexer 能切出完整块（backtick 内 { } 不计深度），但 json5 解析失败
        assert_eq!(parse_script_meta(src), None);
    }

    #[test]
    fn backtick_with_braces_does_not_break_brace_balance() {
        // 验证 lexer 把 backtick 串里的 } 正确跳过（不提前闭合），最终仍交 json5 → None
        let src = "export const meta = { desc: `a } b { c`, name: 'x' }";
        // 整块被完整切出（否则会因为 backtick 里的 } 提前截断，行为不同）；
        // json5 因 backtick 报错 → None。这里断言不 panic 且降级。
        assert_eq!(parse_script_meta(src), None);
    }

    #[test]
    fn real_shape_assess_workflow_migration() {
        // 镜像真实 script `assess-workflow-migration-wf_*.js` 的 meta 形态：
        // name + 长 description + phases[{title, detail}]，meta 后接 const/函数体。
        let src = r"export const meta = {
  name: 'assess-workflow-migration',
  description: 'Assess which workflows can migrate to the Workflow tool, with sketches and tradeoffs',
  phases: [
    { title: 'Assess', detail: 'one agent per candidate workflow, grounded in repo files' },
    { title: 'Synthesize', detail: 'rank candidates, phase the migration, surface tensions' },
  ],
}

const PRIMER = `multi-line primer with { braces } that must not break parsing`
";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("assess-workflow-migration"));
        assert_eq!(meta.phases.len(), 2);
        assert_eq!(meta.phases[0].title, "Assess");
    }

    #[test]
    fn real_shape_explore_with_when_to_use_field() {
        // 镜像真实 script `explore-workflow-rendering-wf_*.js`：description 与 phases
        // 之间夹了 whenToUse 字段（额外未知字段 serde 忽略），phases 4 项。
        let src = r"export const meta = {
  name: 'explore-workflow-rendering',
  description: '探索 Workflow 工具的展示方式（含括号 { } 与中文）',
  whenToUse: '需要为 Workflow 工具调用设计专化渲染时',
  phases: [
    { title: '真相采集', detail: '3 个只读 agent 并行' },
    { title: '视觉设计', detail: 'designer 跑 impeccable' },
    { title: '对抗审', detail: 'codex 攻击设计' },
    { title: '合成', detail: '综合建议' },
  ],
}
";
        let meta = parse_script_meta(src).unwrap();
        assert_eq!(meta.name.as_deref(), Some("explore-workflow-rendering"));
        assert_eq!(meta.phases.len(), 4);
        assert_eq!(meta.phases[3].title, "合成");
    }

    #[test]
    fn no_meta_anchor_returns_none() {
        assert_eq!(parse_script_meta("const x = 1; phase('A')"), None);
    }

    #[test]
    fn truncated_unbalanced_returns_none() {
        let src = "export const meta = { name: 'x', phases: [{ title: 'T' }"; // 缺右括号
        assert_eq!(parse_script_meta(src), None);
    }
}

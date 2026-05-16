//! Session "in progress" 状态检测。
//!
//! 端口自 `../claude-devtools/src/main/utils/sessionStateDetection.ts`。
//! Spec：`openspec/specs/session-display/spec.md` §"Ongoing banner at
//! session bottom" + `openspec/specs/sidebar-navigation/spec.md`
//! §"Ongoing indicator on session item" + `openspec/specs/ipc-data-api/spec.md`
//! §"`extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量"。
//!
//! 算法（流式版本，主路径）：维护单 `ongoing: bool` + `shutdown_tool_ids`
//! 集合，按时间序处理每条消息的 block：AI 活动事件（`Thinking` 非空 / 普通
//! `ToolUse` / 非 rejection 非 shutdown 的 `ToolResult`）置 `ongoing = true`，
//! ending 事件（非空 `Text` / `Interruption` category / `[Request interrupted`
//! 文本前缀 / `ExitPlanMode` / `shutdown_response` 起停 / rejection `ToolResult`）
//! 置 `ongoing = false`。最终 `ongoing` 即为答案——等价于"找最后 ending
//! 之后是否还有 AI 活动"（详见 change `metadata-streaming-ongoing` design D4）。
//!
//! 算法（活动栈版本，oracle）：保留在 `#[cfg(test)] mod oracle`，作为
//! round-trip property test 的等价基准。

use std::collections::HashSet;

use cdt_core::{ContentBlock, MessageCategory, MessageContent, MessageType, ParsedMessage};

const INTERRUPT_PREFIX: &str = "[Request interrupted by user";

/// 增量状态机：流式判定一段消息序列是否仍在进行。
///
/// 用法：`new()` → 反复 `feed(&msg)` → `finalize()` 拿 bool。状态字段：
/// 单 `ongoing: bool` + `shutdown_tool_ids: HashSet<String>`（追踪
/// `SendMessage shutdown_response approve=true` 的 `tool_use_id`，让后续
/// user 消息看到对应 `tool_result` 时正确归类为 Interruption）。
///
/// 等价性：`finalize()` 结果与 `check_messages_ongoing(&messages)` 在
/// 任意有限消息序列上完全一致——后者就是此 SM 的 thin wrapper（见
/// `check_messages_ongoing` 实现）。
pub struct IsOngoingStateMachine {
    ongoing: bool,
    shutdown_tool_ids: HashSet<String>,
}

impl Default for IsOngoingStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl IsOngoingStateMachine {
    /// 构造空状态机。
    #[must_use]
    pub fn new() -> Self {
        Self {
            ongoing: false,
            shutdown_tool_ids: HashSet::new(),
        }
    }

    /// 吃一条消息，按 `MessageType` 分发并按事件类型更新 `ongoing`。
    pub fn feed(&mut self, msg: &ParsedMessage) {
        match msg.message_type {
            MessageType::Assistant => self.process_assistant(msg),
            MessageType::User => self.process_user(msg),
            _ => {}
        }
    }

    /// 消费状态机，返回最终 `is_ongoing` 判定。
    #[must_use]
    pub fn finalize(self) -> bool {
        self.ongoing
    }

    fn process_assistant(&mut self, msg: &ParsedMessage) {
        let MessageContent::Blocks(blocks) = &msg.content else {
            return;
        };
        for block in blocks {
            match block {
                ContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                    self.ongoing = true;
                }
                ContentBlock::Text { text } if !text.trim().is_empty() => {
                    self.ongoing = false;
                }
                ContentBlock::ToolUse { id, name, input } => {
                    if name == "ExitPlanMode" {
                        self.ongoing = false;
                    } else if is_shutdown_response(name, input) {
                        self.shutdown_tool_ids.insert(id.clone());
                        self.ongoing = false;
                    } else {
                        self.ongoing = true;
                    }
                }
                _ => {}
            }
        }
    }

    fn process_user(&mut self, msg: &ParsedMessage) {
        if msg.category == MessageCategory::Interruption {
            self.ongoing = false;
            return;
        }

        let MessageContent::Blocks(blocks) = &msg.content else {
            return;
        };

        let is_rejection = matches!(
            msg.tool_use_result.as_ref().and_then(|v| v.as_str()),
            Some("User rejected tool use")
        );

        for block in blocks {
            match block {
                ContentBlock::ToolResult { tool_use_id, .. } => {
                    self.ongoing = !(self.shutdown_tool_ids.contains(tool_use_id) || is_rejection);
                }
                ContentBlock::Text { text } if text.trim_start().starts_with(INTERRUPT_PREFIX) => {
                    self.ongoing = false;
                }
                _ => {}
            }
        }
    }
}

fn is_shutdown_response(name: &str, input: &serde_json::Value) -> bool {
    if name != "SendMessage" {
        return false;
    }
    let Some(obj) = input.as_object() else {
        return false;
    };
    let is_shutdown = obj
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "shutdown_response");
    let approved = obj
        .get("approve")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    is_shutdown && approved
}

/// 给定消息序列判定会话是否仍在进行。
///
/// 公开 API 不变（spec `ipc-data-api/spec.md` §`extract_session_metadata 公开签名保持纯函数语义`
/// 间接保护）。内部委托给 `IsOngoingStateMachine`——空序列返回 `false`。
//
// 不加 `#[must_use]`：本仓 workspace `unused_must_use = "deny"`，给一个**已存在的**
// `pub` 函数追加 `#[must_use]` 会让忽略返回值的下游调用从编译通过变为 deny lint，
// 等价源级破坏。新增的 SM 接口 `IsOngoingStateMachine::new` / `finalize` 因为是
// 新 API 不受此约束。
pub fn check_messages_ongoing(messages: &[ParsedMessage]) -> bool {
    let mut sm = IsOngoingStateMachine::new();
    for msg in messages {
        sm.feed(msg);
    }
    sm.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{ContentBlock, MessageCategory, MessageContent, MessageType};
    use chrono::{DateTime, Duration, Utc};

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn blank(uuid: &str, n: i64, ty: MessageType, cat: MessageCategory) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.into(),
            parent_uuid: None,
            message_type: ty,
            category: cat,
            timestamp: ts(n),
            role: None,
            content: MessageContent::Blocks(Vec::new()),
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
            tool_use_result: None,
        }
    }

    fn assistant_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(blocks),
            ..blank(uuid, n, MessageType::Assistant, MessageCategory::Assistant)
        }
    }

    fn user_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(blocks),
            ..blank(uuid, n, MessageType::User, MessageCategory::User)
        }
    }

    #[test]
    fn empty_messages_is_not_ongoing() {
        assert!(!check_messages_ongoing(&[]));
    }

    #[test]
    fn plain_text_output_ends_session() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::Text {
                text: "done".into(),
            }],
        )];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn tool_use_after_text_means_ongoing() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::Text {
                    text: "working".into(),
                }],
            ),
            assistant_blocks(
                "a2",
                2,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
        ];
        assert!(check_messages_ongoing(&msgs));
    }

    #[test]
    fn interrupt_marker_user_text_ends_session() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user for tool use]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
        ];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn tool_rejection_ends_session() {
        let mut rejection = user_blocks(
            "u1",
            2,
            vec![ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: serde_json::json!("..."),
                is_error: false,
            }],
        );
        rejection.tool_use_result =
            Some(serde_json::Value::String("User rejected tool use".into()));
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            rejection,
        ];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn exit_plan_mode_is_ending() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "ExitPlanMode".into(),
                input: serde_json::json!({}),
            }],
        )];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn shutdown_response_ends_session_with_matching_result() {
        let shutdown_assistant = assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t-shutdown".into(),
                name: "SendMessage".into(),
                input: serde_json::json!({"type": "shutdown_response", "approve": true}),
            }],
        );
        let shutdown_result = user_blocks(
            "u1",
            2,
            vec![ContentBlock::ToolResult {
                tool_use_id: "t-shutdown".into(),
                content: serde_json::json!("ok"),
                is_error: false,
            }],
        );
        let msgs = vec![shutdown_assistant, shutdown_result];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn ongoing_when_only_ai_activity() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }],
        )];
        assert!(check_messages_ongoing(&msgs));
    }

    #[test]
    fn ai_activity_after_interrupt_resumes_ongoing() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
            assistant_blocks(
                "a2",
                3,
                vec![ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
        ];
        assert!(check_messages_ongoing(&msgs));
    }

    // ========================================================================
    // round-trip property test：SM 流式 vs activity-stack oracle 切片
    //
    // Spec：`openspec/specs/ipc-data-api/spec.md`
    // §`extract_session_metadata 流式判定 isOngoing` Scenario
    // "状态机与切片版 check_messages_ongoing 结果等价"。
    //
    // 6 类典型 fixture + 4 类边界 fixture 全跑 SM 与 oracle，断言相同。
    // ========================================================================

    fn fixture_normal_completed() -> Vec<ParsedMessage> {
        vec![
            user_blocks(
                "u1",
                1,
                vec![ContentBlock::Text {
                    text: "what is rust".into(),
                }],
            ),
            assistant_blocks(
                "a1",
                2,
                vec![ContentBlock::Text {
                    text: "Rust is a systems language.".into(),
                }],
            ),
        ]
    }

    fn fixture_ongoing_tool_use() -> Vec<ParsedMessage> {
        vec![
            user_blocks(
                "u1",
                1,
                vec![ContentBlock::Text {
                    text: "run ls".into(),
                }],
            ),
            assistant_blocks(
                "a1",
                2,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({"command":"ls"}),
                }],
            ),
        ]
    }

    fn fixture_interrupted() -> Vec<ParsedMessage> {
        vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({"command":"sleep 999"}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
        ]
    }

    fn fixture_teammate_message() -> Vec<ParsedMessage> {
        // teammate 消息保持 User category（chunk-building 处理 teammate 嵌入；
        // ongoing 判定不区分 teammate）；后跟 assistant tool_use → 末位 AI → ongoing
        vec![
            user_blocks(
                "u1",
                1,
                vec![ContentBlock::Text {
                    text: r#"<teammate-message teammate_id="alice">help</teammate-message>"#.into(),
                }],
            ),
            assistant_blocks(
                "a1",
                2,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path":"/x"}),
                }],
            ),
        ]
    }

    fn fixture_shutdown_response() -> Vec<ParsedMessage> {
        vec![
            user_blocks(
                "u1",
                1,
                vec![ContentBlock::Text {
                    text: "stop".into(),
                }],
            ),
            assistant_blocks(
                "a1",
                2,
                vec![ContentBlock::ToolUse {
                    id: "t-shutdown".into(),
                    name: "SendMessage".into(),
                    input: serde_json::json!({"type":"shutdown_response","approve":true}),
                }],
            ),
            user_blocks(
                "u2",
                3,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "t-shutdown".into(),
                    content: serde_json::json!("ok"),
                    is_error: false,
                }],
            ),
        ]
    }

    fn fixture_resumed_after_interrupt() -> Vec<ParsedMessage> {
        vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
            assistant_blocks(
                "a2",
                3,
                vec![ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
        ]
    }

    // ---- codex 二审建议补充：4 类 multi-block / 混合场景 fixture ----

    fn fixture_multi_tooluse_text_mixed() -> Vec<ParsedMessage> {
        // 一条 assistant message 含多 ToolUse + 多 Text 混合：按块顺序处理后
        // 末位 block 决定 ongoing。这里末位是 Text（非空）→ ending → false
        vec![assistant_blocks(
            "a1",
            1,
            vec![
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::Text {
                    text: "step 1 done".into(),
                },
                ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::Text {
                    text: "all done.".into(),
                },
            ],
        )]
    }

    fn fixture_thinking_only() -> Vec<ParsedMessage> {
        // assistant message 只含非空 Thinking → AI 活动 → ongoing=true
        vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::Thinking {
                thinking: "let me think...".into(),
                signature: String::new(),
            }],
        )]
    }

    fn fixture_shutdown_plus_other_tool_results() -> Vec<ParsedMessage> {
        // user message 含混合 ToolResult：一个匹配 shutdown 的 + 一个普通的。
        // 按块顺序：第一个匹配 shutdown → ongoing=false；第二个普通 → ongoing=true。
        // 末位 block 决定 → ongoing=true（普通 ToolResult）。
        vec![
            assistant_blocks(
                "a1",
                1,
                vec![
                    ContentBlock::ToolUse {
                        id: "t-shutdown".into(),
                        name: "SendMessage".into(),
                        input: serde_json::json!({"type":"shutdown_response","approve":true}),
                    },
                    ContentBlock::ToolUse {
                        id: "t-bash".into(),
                        name: "Bash".into(),
                        input: serde_json::json!({}),
                    },
                ],
            ),
            user_blocks(
                "u1",
                2,
                vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "t-shutdown".into(),
                        content: serde_json::json!("ok"),
                        is_error: false,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: "t-bash".into(),
                        content: serde_json::json!("ls output"),
                        is_error: false,
                    },
                ],
            ),
        ]
    }

    fn fixture_rejection_multi_tool_results() -> Vec<ParsedMessage> {
        // is_rejection=true 的 user message 含多 ToolResult：所有 ToolResult 都
        // 应归 Interruption (ending) → 末位也是 ending → ongoing=false
        let mut rej = user_blocks(
            "u1",
            2,
            vec![
                ContentBlock::ToolResult {
                    tool_use_id: "t1".into(),
                    content: serde_json::json!("..."),
                    is_error: false,
                },
                ContentBlock::ToolResult {
                    tool_use_id: "t2".into(),
                    content: serde_json::json!("..."),
                    is_error: false,
                },
            ],
        );
        rej.tool_use_result = Some(serde_json::Value::String("User rejected tool use".into()));
        vec![
            assistant_blocks(
                "a1",
                1,
                vec![
                    ContentBlock::ToolUse {
                        id: "t1".into(),
                        name: "Bash".into(),
                        input: serde_json::json!({}),
                    },
                    ContentBlock::ToolUse {
                        id: "t2".into(),
                        name: "Read".into(),
                        input: serde_json::json!({}),
                    },
                ],
            ),
            rej,
        ]
    }

    fn run_sm(messages: &[ParsedMessage]) -> bool {
        let mut sm = IsOngoingStateMachine::new();
        for msg in messages {
            sm.feed(msg);
        }
        sm.finalize()
    }

    fn run_oracle(messages: &[ParsedMessage]) -> bool {
        let activities = oracle::build_activity_stack(messages);
        oracle::is_ongoing_from_activities(&activities)
    }

    #[test]
    fn round_trip_six_typical_fixtures() {
        let fixtures: Vec<(&str, Vec<ParsedMessage>, bool)> = vec![
            ("normal_completed", fixture_normal_completed(), false),
            ("ongoing_tool_use", fixture_ongoing_tool_use(), true),
            ("interrupted", fixture_interrupted(), false),
            ("teammate_message", fixture_teammate_message(), true),
            ("shutdown_response", fixture_shutdown_response(), false),
            (
                "resumed_after_interrupt",
                fixture_resumed_after_interrupt(),
                true,
            ),
        ];
        for (name, msgs, expected) in fixtures {
            let sm = run_sm(&msgs);
            let oracle = run_oracle(&msgs);
            assert_eq!(sm, oracle, "SM vs oracle mismatch for fixture {name}");
            assert_eq!(sm, expected, "expected mismatch for fixture {name}");
            // 同时验证公开 API check_messages_ongoing 也走 SM 路径
            assert_eq!(check_messages_ongoing(&msgs), expected, "{name}");
        }
    }

    #[test]
    fn round_trip_multi_block_mixed_fixtures() {
        // 覆盖 codex 二审建议的 4 类 multi-block / 混合场景。
        let cases: Vec<(&str, Vec<ParsedMessage>, bool)> = vec![
            (
                "multi_tooluse_text_mixed",
                fixture_multi_tooluse_text_mixed(),
                false, // 末位 Text → ending
            ),
            ("thinking_only", fixture_thinking_only(), true),
            (
                "shutdown_plus_other_tool_results",
                fixture_shutdown_plus_other_tool_results(),
                true, // 末位是普通 ToolResult → AI 活动
            ),
            (
                "rejection_multi_tool_results",
                fixture_rejection_multi_tool_results(),
                false, // 所有 ToolResult 都归 Interruption
            ),
        ];
        for (name, msgs, expected) in cases {
            let sm = run_sm(&msgs);
            let oracle = run_oracle(&msgs);
            assert_eq!(sm, oracle, "SM vs oracle mismatch for fixture {name}");
            assert_eq!(sm, expected, "expected mismatch for fixture {name}");
            assert_eq!(check_messages_ongoing(&msgs), expected, "{name}");
        }
    }

    #[test]
    fn round_trip_boundary_fixtures() {
        // 边界 fixture：空 / 单 user / 单 assistant text / 单 assistant tool_use / 全 sidechain
        let cases: Vec<(&str, Vec<ParsedMessage>, bool)> = vec![
            ("empty", vec![], false),
            (
                "lone_user",
                vec![user_blocks(
                    "u1",
                    1,
                    vec![ContentBlock::Text { text: "hi".into() }],
                )],
                false, // 无 AI 活动 → false
            ),
            (
                "lone_assistant_text",
                vec![assistant_blocks(
                    "a1",
                    1,
                    vec![ContentBlock::Text {
                        text: "hello".into(),
                    }],
                )],
                false,
            ),
            (
                "lone_assistant_tool_use",
                vec![assistant_blocks(
                    "a1",
                    1,
                    vec![ContentBlock::ToolUse {
                        id: "t1".into(),
                        name: "Bash".into(),
                        input: serde_json::json!({}),
                    }],
                )],
                true,
            ),
            (
                "all_sidechain",
                vec![ParsedMessage {
                    is_sidechain: true,
                    content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                        id: "t1".into(),
                        name: "Bash".into(),
                        input: serde_json::json!({}),
                    }]),
                    ..blank("a1", 1, MessageType::Assistant, MessageCategory::Assistant)
                }],
                // sidechain 标记不影响 ongoing 判定（与 build_activity_stack 行为一致——
                // 它不检查 is_sidechain）
                true,
            ),
        ];
        for (name, msgs, expected) in cases {
            let sm = run_sm(&msgs);
            let oracle = run_oracle(&msgs);
            assert_eq!(sm, oracle, "SM vs oracle mismatch for boundary {name}");
            assert_eq!(sm, expected, "expected mismatch for boundary {name}");
        }
    }

    // ========================================================================
    // oracle：原 activity-stack 算法，作为 round-trip property test 的等价基准。
    //
    // 设计：见 change `metadata-streaming-ongoing` design.md D5。oracle 已被
    // 9 条单元测试 + 历史 archive `session-ongoing-stale-check` 验证；保留它
    // 让 round-trip test 能在 SM 改动时立即抓出回归。oracle 不公开 API，
    // 仅 `#[cfg(test)]` 编译。
    // ========================================================================

    mod oracle {
        use super::*;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum Activity {
            Thinking,
            ToolUse,
            ToolResult,
            TextOutput,
            Interruption,
            ExitPlanMode,
        }

        impl Activity {
            fn is_ending(self) -> bool {
                matches!(
                    self,
                    Activity::TextOutput | Activity::Interruption | Activity::ExitPlanMode
                )
            }

            fn is_ai(self) -> bool {
                matches!(
                    self,
                    Activity::Thinking | Activity::ToolUse | Activity::ToolResult
                )
            }
        }

        pub(super) fn build_activity_stack(messages: &[ParsedMessage]) -> Vec<Activity> {
            let mut acts: Vec<Activity> = Vec::new();
            let mut shutdown_tool_ids: HashSet<String> = HashSet::new();

            for msg in messages {
                match msg.message_type {
                    MessageType::Assistant => {
                        process_assistant(msg, &mut acts, &mut shutdown_tool_ids);
                    }
                    MessageType::User => process_user(msg, &mut acts, &shutdown_tool_ids),
                    _ => {}
                }
            }

            acts
        }

        fn process_assistant(
            msg: &ParsedMessage,
            acts: &mut Vec<Activity>,
            shutdown_tool_ids: &mut HashSet<String>,
        ) {
            let MessageContent::Blocks(blocks) = &msg.content else {
                return;
            };
            for block in blocks {
                match block {
                    ContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                        acts.push(Activity::Thinking);
                    }
                    ContentBlock::Text { text } if !text.trim().is_empty() => {
                        acts.push(Activity::TextOutput);
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        if name == "ExitPlanMode" {
                            acts.push(Activity::ExitPlanMode);
                        } else if super::is_shutdown_response(name, input) {
                            shutdown_tool_ids.insert(id.clone());
                            acts.push(Activity::Interruption);
                        } else {
                            acts.push(Activity::ToolUse);
                        }
                    }
                    _ => {}
                }
            }
        }

        fn process_user(
            msg: &ParsedMessage,
            acts: &mut Vec<Activity>,
            shutdown_tool_ids: &HashSet<String>,
        ) {
            if msg.category == MessageCategory::Interruption {
                acts.push(Activity::Interruption);
                return;
            }

            let MessageContent::Blocks(blocks) = &msg.content else {
                return;
            };

            let is_rejection = matches!(
                msg.tool_use_result.as_ref().and_then(|v| v.as_str()),
                Some("User rejected tool use")
            );

            for block in blocks {
                match block {
                    ContentBlock::ToolResult { tool_use_id, .. } => {
                        if shutdown_tool_ids.contains(tool_use_id) || is_rejection {
                            acts.push(Activity::Interruption);
                        } else {
                            acts.push(Activity::ToolResult);
                        }
                    }
                    ContentBlock::Text { text }
                        if text.trim_start().starts_with(INTERRUPT_PREFIX) =>
                    {
                        acts.push(Activity::Interruption);
                    }
                    _ => {}
                }
            }
        }

        pub(super) fn is_ongoing_from_activities(activities: &[Activity]) -> bool {
            if activities.is_empty() {
                return false;
            }

            let last_ending = activities.iter().rposition(|a| a.is_ending());

            match last_ending {
                None => activities.iter().any(|a| a.is_ai()),
                Some(idx) => activities[idx + 1..].iter().any(|a| a.is_ai()),
            }
        }
    }
}

//! 从 `AIChunk` 的 assistant 响应中按块顺序抽取语义步骤。
//!
//! Spec scenario："AIChunk with thinking + text + tool"。
//!
//! `SubagentSpawn` 变体在本 capability 下不会产出；当
//! `team-coordination-metadata` port 完成后，那次 port 会在此处补上产出逻辑。

use cdt_core::{AssistantResponse, ContentBlock, MessageContent, SemanticStep};

pub fn extract_semantic_steps(responses: &[AssistantResponse]) -> Vec<SemanticStep> {
    let mut out = Vec::new();
    for r in responses {
        let MessageContent::Blocks(blocks) = &r.content else {
            continue;
        };
        for block in blocks {
            match block {
                ContentBlock::Thinking { thinking, .. } => {
                    if !thinking.is_empty() {
                        out.push(SemanticStep::Thinking {
                            text: thinking.clone(),
                            timestamp: r.timestamp,
                        });
                    }
                }
                ContentBlock::Text { text } => {
                    if !text.is_empty() {
                        out.push(SemanticStep::Text {
                            text: text.clone(),
                            timestamp: r.timestamp,
                        });
                    }
                }
                ContentBlock::ToolUse { id, name, .. } => {
                    out.push(SemanticStep::ToolExecution {
                        tool_use_id: id.clone(),
                        tool_name: name.clone(),
                        timestamp: r.timestamp,
                    });
                }
                ContentBlock::ToolResult { .. }
                | ContentBlock::Image { .. }
                | ContentBlock::Unknown => {}
            }
        }
    }
    out
}

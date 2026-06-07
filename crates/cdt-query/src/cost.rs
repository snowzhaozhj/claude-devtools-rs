//! Token 费用估算。
//!
//! 价格表内置常量，按 model ID 前缀匹配。
//! 数据来源：<https://docs.anthropic.com/en/docs/about-claude/pricing>（2026-05）

use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub model_prefix: &'static str,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
}

pub const PRICING_TABLE: &[ModelPricing] = &[
    ModelPricing {
        model_prefix: "claude-opus-4",
        input_per_mtok: 5.0,
        output_per_mtok: 25.0,
        cache_read_per_mtok: 0.50,
        cache_write_per_mtok: 6.25,
    },
    ModelPricing {
        model_prefix: "claude-sonnet-4",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
    },
    ModelPricing {
        model_prefix: "claude-haiku-4",
        input_per_mtok: 1.0,
        output_per_mtok: 5.0,
        cache_read_per_mtok: 0.10,
        cache_write_per_mtok: 1.25,
    },
    ModelPricing {
        model_prefix: "claude-3-7-sonnet",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
    },
    ModelPricing {
        model_prefix: "claude-3-5-sonnet",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
    },
    ModelPricing {
        model_prefix: "claude-3-5-haiku",
        input_per_mtok: 1.0,
        output_per_mtok: 5.0,
        cache_read_per_mtok: 0.10,
        cache_write_per_mtok: 1.25,
    },
    ModelPricing {
        model_prefix: "claude-3-haiku",
        input_per_mtok: 0.25,
        output_per_mtok: 1.25,
        cache_read_per_mtok: 0.025,
        cache_write_per_mtok: 0.3125,
    },
    ModelPricing {
        model_prefix: "claude-3-opus",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
    },
];

const DEFAULT_PRICING: ModelPricing = ModelPricing {
    model_prefix: "unknown",
    input_per_mtok: 3.0,
    output_per_mtok: 15.0,
    cache_read_per_mtok: 0.30,
    cache_write_per_mtok: 3.75,
};

pub fn lookup_pricing(model: &str) -> &'static ModelPricing {
    for p in PRICING_TABLE {
        if model.starts_with(p.model_prefix) {
            return p;
        }
    }
    &DEFAULT_PRICING
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_tokens: u64,
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_creation_cost: f64,
    pub total_cost: f64,
    pub model: String,
    pub model_pricing_used: String,
}

pub fn compute_session_cost(detail: &cdt_api::SessionDetail) -> SessionCost {
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;
    let mut input_cost: f64 = 0.0;
    let mut output_cost: f64 = 0.0;
    let mut cache_read_cost: f64 = 0.0;
    let mut cache_creation_cost: f64 = 0.0;
    let mut primary_model: Option<String> = None;

    for chunk in &detail.chunks {
        if let cdt_core::Chunk::Ai(ai) = chunk {
            for resp in &ai.responses {
                if let Some(ref usage) = resp.usage {
                    let model_id = resp.model.as_deref().unwrap_or("unknown");
                    let pricing = lookup_pricing(model_id);

                    input_tokens = input_tokens.saturating_add(usage.input_tokens);
                    output_tokens = output_tokens.saturating_add(usage.output_tokens);
                    cache_read_tokens =
                        cache_read_tokens.saturating_add(usage.cache_read_input_tokens);
                    cache_creation_tokens =
                        cache_creation_tokens.saturating_add(usage.cache_creation_input_tokens);

                    #[allow(clippy::cast_precision_loss)]
                    {
                        input_cost +=
                            usage.input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
                        output_cost +=
                            usage.output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
                        cache_read_cost += usage.cache_read_input_tokens as f64
                            * pricing.cache_read_per_mtok
                            / 1_000_000.0;
                        cache_creation_cost += usage.cache_creation_input_tokens as f64
                            * pricing.cache_write_per_mtok
                            / 1_000_000.0;
                    }

                    if primary_model.is_none() {
                        primary_model.clone_from(&resp.model);
                    }
                }
            }
        }
    }

    let model = primary_model.unwrap_or_else(|| "unknown".to_string());
    let total_tokens = input_tokens
        .saturating_add(output_tokens)
        .saturating_add(cache_read_tokens)
        .saturating_add(cache_creation_tokens);
    let total_cost = input_cost + output_cost + cache_read_cost + cache_creation_cost;

    SessionCost {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        total_tokens,
        input_cost,
        output_cost,
        cache_read_cost,
        cache_creation_cost,
        total_cost,
        model_pricing_used: lookup_pricing(&model).model_prefix.to_string(),
        model,
    }
}

pub fn compute_cost_from_usage(usage: &cdt_core::TokenUsage, model: &str) -> SessionCost {
    let pricing = lookup_pricing(model);
    let input_tokens = usage.input_tokens;
    let output_tokens = usage.output_tokens;
    let cache_read_tokens = usage.cache_read_input_tokens;
    let cache_creation_tokens = usage.cache_creation_input_tokens;

    #[allow(clippy::cast_precision_loss)]
    let input_cost = input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
    #[allow(clippy::cast_precision_loss)]
    let output_cost = output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
    #[allow(clippy::cast_precision_loss)]
    let cache_read_cost = cache_read_tokens as f64 * pricing.cache_read_per_mtok / 1_000_000.0;
    #[allow(clippy::cast_precision_loss)]
    let cache_creation_cost =
        cache_creation_tokens as f64 * pricing.cache_write_per_mtok / 1_000_000.0;

    let total_tokens = input_tokens
        .saturating_add(output_tokens)
        .saturating_add(cache_read_tokens)
        .saturating_add(cache_creation_tokens);
    let total_cost = input_cost + output_cost + cache_read_cost + cache_creation_cost;

    SessionCost {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        total_tokens,
        input_cost,
        output_cost,
        cache_read_cost,
        cache_creation_cost,
        total_cost,
        model_pricing_used: pricing.model_prefix.to_string(),
        model: model.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_opus_pricing() {
        let p = lookup_pricing("claude-opus-4-7-20260501");
        assert_eq!(p.model_prefix, "claude-opus-4");
        assert!((p.input_per_mtok - 5.0).abs() < f64::EPSILON);
        assert!((p.output_per_mtok - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_sonnet_pricing() {
        let p = lookup_pricing("claude-sonnet-4-6-20260401");
        assert_eq!(p.model_prefix, "claude-sonnet-4");
        assert!((p.input_per_mtok - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_haiku_pricing() {
        let p = lookup_pricing("claude-haiku-4-5-20251001");
        assert_eq!(p.model_prefix, "claude-haiku-4");
        assert!((p.input_per_mtok - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_unknown_falls_back() {
        let p = lookup_pricing("gpt-4o");
        assert_eq!(p.model_prefix, "unknown");
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn cost_calculation_basic() {
        let pricing = lookup_pricing("claude-sonnet-4-6");
        let input = 1_000_000u64;
        let cost = input as f64 * pricing.input_per_mtok / 1_000_000.0;
        assert!((cost - 3.0).abs() < f64::EPSILON);
    }
}

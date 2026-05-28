/// Token estimation for context budget control.
///
/// Default implementation uses character-count heuristic (~4 chars/token).
/// Trait allows swapping to tiktoken/cl100k in the future.
pub trait TokenEstimator: Send + Sync {
    fn estimate(&self, text: &str) -> usize;
}

pub struct CharRatioEstimator {
    chars_per_token: f64,
}

impl CharRatioEstimator {
    #[must_use]
    pub fn new(chars_per_token: f64) -> Self {
        Self { chars_per_token }
    }
}

impl Default for CharRatioEstimator {
    fn default() -> Self {
        Self::new(4.0)
    }
}

impl TokenEstimator for CharRatioEstimator {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn estimate(&self, text: &str) -> usize {
        (text.len() as f64 / self.chars_per_token).ceil() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_ratio_estimator_basic() {
        let e = CharRatioEstimator::default();
        assert_eq!(e.estimate(""), 0);
        assert_eq!(e.estimate("abcd"), 1);
        assert_eq!(e.estimate("abcde"), 2);
        assert_eq!(e.estimate("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn custom_ratio() {
        let e = CharRatioEstimator::new(2.0);
        assert_eq!(e.estimate("abcd"), 2);
    }
}

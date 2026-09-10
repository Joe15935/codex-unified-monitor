use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Canonical accounting. Cached input is a subset of input; reasoning is a subset of output.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub raw_input_tokens: u64,
    pub cached_input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub cache_write_input_tokens: u64,
}
impl Tokens {
    pub fn new(
        raw: u64,
        cached: u64,
        output: u64,
        reasoning: u64,
        write: u64,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(cached <= raw, "cached_input_exceeds_raw");
        anyhow::ensure!(reasoning <= output, "reasoning_exceeds_output");
        anyhow::ensure!(write <= raw, "cache_write_exceeds_raw");
        let total = raw
            .checked_add(output)
            .ok_or_else(|| anyhow::anyhow!("token_overflow"))?;
        anyhow::ensure!(total <= i64::MAX as u64, "token_overflow");
        Ok(Self {
            raw_input_tokens: raw,
            cached_input_tokens: cached,
            uncached_input_tokens: raw - cached,
            output_tokens: output,
            reasoning_output_tokens: reasoning,
            total_tokens: total,
            cache_write_input_tokens: write,
        })
    }
    pub fn from_usage(v: &Value) -> anyhow::Result<Self> {
        let n = |key| -> anyhow::Result<u64> {
            match v.get(key) {
                None | Some(Value::Null) => Ok(0),
                Some(x) => x
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("invalid_token_counter")),
            }
        };
        Self::new(
            n("input_tokens")?,
            n("cached_input_tokens")?,
            n("output_tokens")?,
            n("reasoning_output_tokens")?,
            n("cache_write_input_tokens")?,
        )
    }
    pub fn add(&mut self, other: &Self) {
        self.raw_input_tokens += other.raw_input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.uncached_input_tokens += other.uncached_input_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_output_tokens += other.reasoning_output_tokens;
        self.total_tokens += other.total_tokens;
        self.cache_write_input_tokens += other.cache_write_input_tokens;
    }
    pub fn cache_ratio(&self) -> Option<f64> {
        (self.raw_input_tokens > 0)
            .then(|| self.cached_input_tokens as f64 / self.raw_input_tokens as f64)
    }
}

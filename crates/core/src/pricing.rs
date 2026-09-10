use crate::{accounting::Tokens, settings::Settings};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub model: String,
    pub input: f64,
    pub cached_input: f64,
    pub output: f64,
    pub cache_write: Option<f64>,
    pub source: String,
    pub effective_date: Option<String>,
    pub verified_at: String,
    pub pricing_mode: String,
    #[serde(default)]
    pub processing: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub notes: String,
}
impl Price {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.model.trim().is_empty(), "Model name required");
        anyhow::ensure!(
            !self.source.trim().is_empty() && !self.verified_at.trim().is_empty(),
            "Price source and verification date required"
        );
        anyhow::ensure!(
            [self.input, self.cached_input, self.output]
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0 && *x < 1_000_000.0),
            "Invalid model rate"
        );
        anyhow::ensure!(
            self.cache_write
                .is_none_or(|x| x.is_finite() && x >= 0.0 && x < 1_000_000.0),
            "Invalid cache-write rate"
        );
        anyhow::ensure!(
            matches!(self.pricing_mode.as_str(), "public_api" | "codex_work"),
            "Invalid price mode"
        );
        Ok(())
    }
    pub fn value(&self, t: &Tokens) -> f64 {
        (t.uncached_input_tokens as f64 * self.input
            + t.cached_input_tokens as f64 * self.cached_input
            + t.output_tokens as f64 * self.output)
            / 1_000_000.0
    }
    pub fn savings(&self, t: &Tokens) -> f64 {
        t.cached_input_tokens as f64 * (self.input - self.cached_input) / 1_000_000.0
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub verified_at: String,
    pub entries: Vec<Price>,
}
impl Catalog {
    pub fn bundled() -> Self {
        serde_json::from_str(include_str!("../../../pricing_catalog.json"))
            .expect("validated bundled catalog")
    }
    pub fn resolve<'a>(
        &'a self,
        model: &str,
        mode: &str,
        settings: &'a Settings,
    ) -> Option<&'a Price> {
        let key = model.trim().to_lowercase();
        let name = settings
            .aliases
            .get(&key)
            .map(|x| x.as_str())
            .unwrap_or(&key);
        settings
            .custom_rates
            .iter()
            .find(|p| p.model == name && p.pricing_mode == mode)
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|p| p.model == name && p.pricing_mode == mode)
            })
    }
}

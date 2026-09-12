use crate::{pricing::Price, storage::Store};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub timezone: String,
    pub quota_poll_seconds: u64,
    pub adaptive_refresh: bool,
    pub low_quota_threshold: u8,
    pub tray_metric: String,
    pub pricing_mode: String,
    pub monthly_subscription_cost: Option<f64>,
    pub cache_thresholds: [f64; 3],
    pub aliases: BTreeMap<String, String>,
    pub custom_rates: Vec<Price>,
    pub account_enabled: bool,
    pub theme: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            timezone: iana_time_zone::get_timezone().unwrap_or("UTC".into()),
            quota_poll_seconds: 90,
            adaptive_refresh: true,
            low_quota_threshold: 10,
            tray_metric: "weekly".into(),
            pricing_mode: "public_api".into(),
            monthly_subscription_cost: None,
            cache_thresholds: [0.9, 0.75, 0.5],
            aliases: BTreeMap::new(),
            custom_rates: vec![],
            account_enabled: true,
            theme: "system".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(self.language.as_str(), "en" | "zh-CN"),
            "Choose English or Simplified Chinese"
        );
        anyhow::ensure!(
            self.low_quota_threshold <= 50,
            "Low quota threshold must be 0–50 percent"
        );
        self.timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| anyhow::anyhow!("Use an IANA time zone, e.g. America/New_York"))?;
        anyhow::ensure!(
            (60..=3600).contains(&self.quota_poll_seconds),
            "Polling must be 60–3600 seconds"
        );
        anyhow::ensure!(
            self.monthly_subscription_cost
                .is_none_or(|x| x.is_finite() && x > 0.0),
            "Subscription price must be positive"
        );
        anyhow::ensure!(
            matches!(
                self.tray_metric.as_str(),
                "five_hour" | "weekly" | "today_value" | "today_tokens"
            ),
            "Invalid tray metric"
        );
        anyhow::ensure!(
            matches!(self.pricing_mode.as_str(), "public_api" | "codex_work"),
            "Invalid pricing mode"
        );
        anyhow::ensure!(
            matches!(self.theme.as_str(), "system" | "light" | "dark"),
            "Invalid theme"
        );
        let t = self.cache_thresholds;
        anyhow::ensure!(
            t.iter().all(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0) && t[0] > t[1] && t[1] > t[2],
            "Cache thresholds must descend from 0 to 1"
        );
        for rate in &self.custom_rates {
            rate.validate()?;
        }
        for (from, to) in &self.aliases {
            anyhow::ensure!(
                !from.trim().is_empty() && !to.trim().is_empty() && from != to,
                "Invalid model alias"
            );
        }
        Ok(())
    }
    pub fn load(store: &Store) -> anyhow::Result<Self> {
        let value: Option<String> = store
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='preferences'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default())
    }
    pub fn save(&self, store: &Store) -> anyhow::Result<()> {
        self.validate()?;
        store.conn.execute("INSERT INTO settings(key,value) VALUES('preferences',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(self)?])?;
        Ok(())
    }
}

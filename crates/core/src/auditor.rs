//! Observational capacity auditing, not entitlement detection or a billing rule.
use crate::{
    account::Quota, accounting::Tokens, digest, pricing::Catalog, settings::Settings,
    storage::Store,
};
use anyhow::{ensure, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const METHOD: &str = "tier-audit-v1";
pub const BOUNDARY: &str = "Capacity resemblance is conditional on the supplied comparison and observed workload. This report cannot establish a backend entitlement error. Account-wide quota can include unobserved clients, cloud tasks and other shared features. API-equivalent values are estimates, not subscription charges.";
pub const SETTLING_SECONDS: i64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Controls {
    pub expected_tier: String,
    pub model: String,
    pub effort: String,
    pub context_band: String,
    pub workload_class: String,
    pub fast_off_attested: bool,
    pub no_subagents_attested: bool,
    pub exclusive_local_use_attested: bool,
}
fn safe_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
impl Controls {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            ["pro_5x", "pro_20x", "unspecified"].contains(&self.expected_tier.as_str()),
            "Invalid expected tier"
        );
        ensure!(
            safe_label(&self.model) && self.model != "unknown",
            "Select one exact model"
        );
        ensure!(
            ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"]
                .contains(&self.effort.as_str()),
            "Select one reasoning effort"
        );
        ensure!(
            ["short", "medium", "long", "extended"].contains(&self.context_band.as_str()),
            "Select an input-context band"
        );
        ensure!(
            ["coding", "research", "writing", "no_tools"].contains(&self.workload_class.as_str()),
            "Select a workload class"
        );
        ensure!(self.fast_off_attested && self.no_subagents_attested && self.exclusive_local_use_attested, "Controlled observation requires explicit Fast-off, no-subagent and exclusive-local-use declarations");
        Ok(())
    }
}
pub fn context_band(input: u64) -> &'static str {
    match input {
        0..=32000 => "short",
        32001..=128000 => "medium",
        128001..=256000 => "long",
        _ => "extended",
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenPricing {
    pub rates: BTreeMap<String, [f64; 3]>,
    pub verified_at: String,
    pub sources: Vec<String>,
    pub fingerprint: String,
}
impl FrozenPricing {
    pub fn capture(c: &Catalog, s: &Settings) -> Self {
        let names: BTreeSet<_> = c
            .entries
            .iter()
            .chain(s.custom_rates.iter())
            .filter(|p| p.pricing_mode == "public_api")
            .map(|p| p.model.clone())
            .chain(s.aliases.keys().cloned())
            .filter(|n| safe_label(n))
            .collect();
        let mut rates = BTreeMap::new();
        let mut sources = BTreeSet::new();
        for name in names {
            if let Some(p) = c.resolve(&name, "public_api", s) {
                rates.insert(name, [p.input, p.cached_input, p.output]);
                // Keep only public, credential-free official price citations.
                if (p.source.starts_with("https://developers.openai.com/")
                    || p.source.starts_with("https://help.openai.com/")
                    || p.source.starts_with("https://learn.chatgpt.com/"))
                    && !p.source.contains(['?', '#', '@', '\n'])
                {
                    sources.insert(p.source.clone());
                } else {
                    sources.insert("User-supplied source omitted for privacy".into());
                }
            }
        }
        let fingerprint = digest(serde_json::to_vec(&rates).unwrap());
        Self {
            rates,
            verified_at: chrono::NaiveDate::parse_from_str(&c.verified_at, "%Y-%m-%d")
                .map(|d| d.to_string())
                .unwrap_or_else(|_| "Unknown".into()),
            sources: sources.into_iter().collect(),
            fingerprint,
        }
    }
    fn value(&self, model: &str, t: &Tokens) -> Option<f64> {
        self.rates.get(model).map(|p| {
            (t.uncached_input_tokens as f64 * p[0]
                + t.cached_input_tokens as f64 * p[1]
                + t.output_tokens as f64 * p[2])
                / 1e6
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunConfig {
    controls: Controls,
    pricing: FrozenPricing,
    max_poll_gap: i64,
    initial_errors: u64,
}
#[derive(Debug, Clone)]
struct Run {
    account: String,
    start: i64,
    end: Option<i64>,
    config: RunConfig,
}
fn latest_run(store: &Store, key: &str) -> Result<Option<Run>> {
    let row: Option<(String,i64,Option<i64>,String)> = store.conn.query_row("SELECT account_key,started_at,ended_at,config FROM audit_runs WHERE account_key=? ORDER BY id DESC LIMIT 1", [key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    row.map(|(account, start, end, c)| {
        Ok(Run {
            account,
            start,
            end,
            config: serde_json::from_str(&c)?,
        })
    })
    .transpose()
}
fn parser_errors(store: &Store) -> Result<u64> {
    Ok(store
        .conn
        .query_row("SELECT COALESCE(SUM(errors),0) FROM files", [], |r| {
            r.get(0)
        })?)
}
pub fn start(
    store: &Store,
    q: &Quota,
    c: &Catalog,
    s: &Settings,
    controls: Controls,
    now: i64,
) -> Result<()> {
    controls.validate()?;
    ensure!(
        q.meta.status == "LIVE"
            && q.meta
                .updated_at
                .is_some_and(|t| (0..=300).contains(&(now - t)))
            && !q.account_key.is_empty()
            && q.weekly.is_some(),
        "A recent official weekly reading is required"
    );
    let active: u64 = store.conn.query_row(
        "SELECT COUNT(*) FROM audit_runs WHERE ended_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    ensure!(
        active == 0,
        "Stop the existing observation before starting another"
    );
    let pricing = FrozenPricing::capture(c, s);
    ensure!(
        pricing.rates.contains_key(&controls.model),
        "The selected model needs a known public API rate"
    );
    let config = RunConfig {
        controls,
        pricing,
        max_poll_gap: (s.quota_poll_seconds as i64 * 3).clamp(300, 900),
        initial_errors: parser_errors(store)?,
    };
    store.conn.execute(
        "INSERT INTO audit_runs(account_key,started_at,config) VALUES(?,?,?)",
        params![q.account_key, now, serde_json::to_string(&config)?],
    )?;
    Ok(())
}
pub fn stop(store: &Store, now: i64) -> Result<()> {
    store.conn.execute(
        "UPDATE audit_runs SET ended_at=? WHERE ended_at IS NULL",
        [now],
    )?;
    Ok(())
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
    pub reset_at: Option<i64>,
    pub weekly_before: Option<f64>,
    pub weekly_after: Option<f64>,
    pub delta_quota: Option<f64>,
    pub tokens: Tokens,
    pub api_equivalent: f64,
    pub unpriced_tokens: u64,
    pub usd_per_percent: Option<f64>,
    pub responses: u64,
    pub workloads: Vec<String>,
    pub tool_calls: u64,
    pub models: Vec<String>,
    pub efforts: Vec<String>,
    pub input_min: u64,
    pub input_max: u64,
    pub fast_evidence: String,
    pub subagent_evidence: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub samples: usize,
    pub workloads: usize,
    pub observation_seconds: i64,
    pub quota_change: f64,
    pub tokens: Tokens,
    pub responses: u64,
    pub tool_calls: u64,
    pub api_equivalent: f64,
    pub usd_per_percent: Option<f64>,
    pub dispersion: Option<f64>,
    pub coefficient_of_variation: Option<f64>,
    pub capacity_per_week: Option<f64>,
    pub capacity_range: Option<[f64; 2]>,
    pub quality: String,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub method: String,
    pub generated_at: i64,
    pub origin_id: String,
    pub reported_plan: String,
    pub controls: Option<Controls>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub pricing: FrozenPricing,
    pub intervals: Vec<Interval>,
    pub summary: Summary,
    pub assessment: String,
    pub confidence: String,
    pub relative_index: Option<f64>,
    pub relative_range: Option<[f64; 2]>,
    pub baseline_capacity: Option<f64>,
    pub baseline_source: Option<String>,
    pub assessment_reasons: Vec<String>,
    pub boundary: String,
    pub methodology: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub evidence: Evidence,
    pub running: bool,
    pub active_elsewhere: bool,
    pub baseline_loaded: bool,
    pub quota_status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub source: String,
    pub evidence: Evidence,
}

fn origin(store: &Store, key: &str) -> Result<String> {
    if key.is_empty() {
        return Ok("unavailable".into());
    }
    let old: Option<String> = store
        .conn
        .query_row(
            "SELECT origin FROM audit_origins WHERE account_key=?",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = old {
        return Ok(id);
    }
    // Opaque per-account export identity; no reversible account hash or path.
    let id = digest(format!(
        "{:?}:{}",
        std::time::SystemTime::now(),
        std::process::id()
    ));
    store.conn.execute(
        "INSERT OR IGNORE INTO audit_origins(account_key,origin) VALUES(?,?)",
        params![key, id],
    )?;
    Ok(store.conn.query_row(
        "SELECT origin FROM audit_origins WHERE account_key=?",
        [key],
        |r| r.get(0),
    )?)
}
fn label(s: &str) -> String {
    if safe_label(s) {
        s.into()
    } else {
        "unknown".into()
    }
}
fn plan(s: Option<&str>) -> String {
    match s {
        Some("pro") => "ChatGPT Pro",
        Some("plus") => "ChatGPT Plus",
        Some("free") => "ChatGPT Free",
        Some("business") => "ChatGPT Business",
        _ => "Unknown",
    }
    .into()
}
fn push_unique(v: &mut Vec<String>, text: &str) {
    if !v.iter().any(|s| s == text) {
        v.push(text.into());
    }
}

fn interval(
    store: &Store,
    a: &Quota,
    b: &Quota,
    pricing: &FrozenPricing,
    run: Option<&Run>,
    mut reasons: Vec<String>,
    now: i64,
    ids: &mut BTreeMap<String, String>,
) -> Result<Interval> {
    let start = a.meta.updated_at.unwrap_or(0);
    let end = b.meta.updated_at.unwrap_or(0);
    let delta = a
        .weekly
        .as_ref()
        .zip(b.weekly.as_ref())
        .map(|(a, b)| b.used_percent - a.used_percent);
    let mut row = Interval {
        start,
        end,
        reset_at: b.weekly.as_ref().and_then(|w| w.resets_at),
        weekly_before: a.weekly.as_ref().map(|w| w.remaining_percent),
        weekly_after: b.weekly.as_ref().map(|w| w.remaining_percent),
        delta_quota: delta,
        input_min: u64::MAX,
        ..Interval::default()
    };
    let mut models = BTreeSet::new();
    let mut efforts = BTreeSet::new();
    let mut workloads = BTreeSet::new();
    let mut unknown_fast = false;
    let mut fast = false;
    let mut unknown_agent = false;
    let mut subagent = false;
    let mut missing_turn = false;
    store.visit_events(start.saturating_add(1), end.saturating_add(1), |e| {
        if e.kind == "tool" {
            row.tool_calls += 1;
            if e.tool.contains("spawn_agent") || e.tool.contains("create_thread") {
                subagent = true;
            }
        }
        if e.kind != "token" {
            return;
        }
        row.tokens.add(&e.tokens);
        row.responses += 1;
        row.input_min = row.input_min.min(e.tokens.raw_input_tokens);
        row.input_max = row.input_max.max(e.tokens.raw_input_tokens);
        if let Some(v) = pricing.value(&e.model, &e.tokens) {
            row.api_equivalent += v;
        } else {
            row.unpriced_tokens += e.tokens.total_tokens;
        }
        models.insert(label(&e.model));
        efforts.insert(label(&e.effort));
        if e.turn_id.is_empty() {
            missing_turn = true;
        } else {
            let key = format!("{}:{}", e.session_id, e.turn_id);
            let next = format!("workload-{:04}", ids.len() + 1);
            workloads.insert(ids.entry(key).or_insert(next).clone());
        }
        match e.service_tier.as_deref() {
            Some("default" | "standard") => {}
            Some("fast" | "priority" | "flex" | "batch") => fast = true,
            _ => unknown_fast = true,
        }
        match e.is_subagent {
            Some(true) => subagent = true,
            None => unknown_agent = true,
            _ => {}
        }
    })?;
    if row.input_min == u64::MAX {
        row.input_min = 0;
    }
    row.models = models.into_iter().collect();
    row.efforts = efforts.into_iter().collect();
    row.workloads = workloads.into_iter().collect();
    row.fast_evidence = if fast {
        "NON_STANDARD"
    } else if unknown_fast {
        "UNKNOWN"
    } else {
        "LOG_VERIFIED"
    }
    .into();
    row.subagent_evidence = if subagent {
        "DETECTED"
    } else if unknown_agent {
        "UNKNOWN"
    } else {
        "LOG_VERIFIED"
    }
    .into();
    if let Some(run) = run {
        let c = &run.config.controls;
        if row.start < run.start || run.end.is_some_and(|t| row.end > t) {
            push_unique(&mut reasons, "Outside controlled observation");
        }
        if row.models != [c.model.clone()] {
            push_unique(&mut reasons, "Mixed or different model");
        }
        if row.efforts != [c.effort.clone()] {
            push_unique(&mut reasons, "Mixed, missing or different reasoning");
        }
        if context_band(row.input_min) != c.context_band
            || context_band(row.input_max) != c.context_band
        {
            push_unique(&mut reasons, "Input context outside selected band");
        }
        if c.workload_class == "no_tools" && row.tool_calls > 0 {
            push_unique(&mut reasons, "Tools used in no-tools protocol");
        }
        if unknown_fast && c.fast_off_attested && !fast {
            row.fast_evidence = "USER_ATTESTED".into();
        }
        if unknown_agent && c.no_subagents_attested && !subagent {
            row.subagent_evidence = "USER_ATTESTED".into();
        }
    } else {
        push_unique(&mut reasons, "No controlled observation");
    }
    if fast {
        push_unique(&mut reasons, "Fast or non-standard service tier detected");
    }
    if subagent {
        push_unique(&mut reasons, "Subagent or delegated task detected");
    }
    if missing_turn {
        push_unique(&mut reasons, "Missing workload identity");
    }
    if row.responses == 0 {
        push_unique(&mut reasons, "No local token evidence");
    }
    if row.unpriced_tokens > 0 {
        push_unique(&mut reasons, "Incomplete pricing coverage");
    }
    if delta.is_none_or(|d| d <= 0.0) {
        push_unique(&mut reasons, "No positive quota change");
    }
    if row.weekly_before.is_some_and(|x| x <= 1.0) || row.weekly_after.is_some_and(|x| x <= 1.0) {
        push_unique(&mut reasons, "Quota saturation or possible paid-credit use");
    }
    if now - end < SETTLING_SECONDS {
        push_unique(
            &mut reasons,
            "Settling: wait 10 minutes for delayed records",
        );
    }
    if let Some(d) = delta.filter(|d| *d > 0.0) {
        if row.unpriced_tokens == 0 && row.responses > 0 {
            row.usd_per_percent = Some(row.api_equivalent / d);
        }
    }
    row.eligible = reasons.is_empty();
    row.reasons = reasons;
    Ok(row)
}

pub fn summarize(rows: &[Interval]) -> Summary {
    let selected: Vec<_> = rows.iter().filter(|r| r.eligible).collect();
    let mut s = Summary {
        quality: "INSUFFICIENT".into(),
        ..Summary::default()
    };
    let mut workloads = BTreeSet::new();
    let mut segments = 0;
    let mut last: Option<&Interval> = None;
    for r in &selected {
        s.samples += 1;
        s.observation_seconds += r.end - r.start;
        s.quota_change += r.delta_quota.unwrap_or(0.0);
        s.tokens.add(&r.tokens);
        s.responses += r.responses;
        s.tool_calls += r.tool_calls;
        s.api_equivalent += r.api_equivalent;
        workloads.extend(r.workloads.iter());
        if last.is_none_or(|p| p.end != r.start || p.reset_at != r.reset_at) {
            segments += 1;
        }
        last = Some(r);
    }
    s.workloads = workloads.len();
    if s.quota_change > 0.0 && s.api_equivalent > 0.0 {
        let mean = s.api_equivalent / s.quota_change;
        let variance = selected
            .iter()
            .map(|r| {
                r.delta_quota.unwrap_or(0.0) * (r.usd_per_percent.unwrap_or(0.0) - mean).powi(2)
            })
            .sum::<f64>()
            / s.quota_change;
        let sd = variance.sqrt();
        s.usd_per_percent = Some(mean);
        s.dispersion = Some(sd);
        s.coefficient_of_variation = Some(sd / mean);
        s.capacity_per_week = Some(100.0 * mean);
        // Conservative assumed ±1 pp per endpoint; internal contiguous endpoints cancel.
        let error = 2.0 * segments as f64;
        if s.quota_change > error {
            s.capacity_range = Some([
                (100.0 * s.api_equivalent / (s.quota_change + error))
                    .min(100.0 * (mean - 2.0 * sd).max(0.0)),
                (100.0 * s.api_equivalent / (s.quota_change - error))
                    .max(100.0 * (mean + 2.0 * sd)),
            ]);
        }
    }
    if s.samples < 12 {
        s.reasons
            .push("At least 12 distinct quota-change windows required".into());
    }
    if s.workloads < 20 {
        s.reasons
            .push("At least 20 distinct workloads required".into());
    }
    if s.observation_seconds < 8 * 3600 {
        s.reasons
            .push("At least 8 hours of eligible coverage required".into());
    }
    if s.quota_change < 20.0 {
        s.reasons
            .push("At least 20 percentage points of eligible consumption required".into());
    }
    if s.coefficient_of_variation.is_none_or(|cv| cv > 0.25) {
        s.reasons
            .push("Consumption rates are missing or unstable (CV > 25%)".into());
    }
    if s.capacity_range.is_none() {
        s.reasons
            .push("Quota resolution is too coarse for the selected evidence".into());
    }
    if s.reasons.is_empty() {
        s.quality = if s.samples >= 30
            && s.workloads >= 30
            && s.observation_seconds >= 24 * 3600
            && s.quota_change >= 30.0
            && selected
                .iter()
                .all(|r| r.fast_evidence == "LOG_VERIFIED" && r.subagent_evidence == "LOG_VERIFIED")
        {
            "HIGH"
        } else {
            "MEDIUM"
        }
        .into();
    }
    s
}

pub fn compare(current: &mut Evidence, baseline: Option<&Baseline>) {
    current.assessment_reasons.clear();
    current.relative_index = None;
    current.relative_range = None;
    current.baseline_capacity = None;
    current.baseline_source = None;
    current.assessment = "INCONCLUSIVE".into();
    current.confidence = "INSUFFICIENT".into();
    let Some(b) = baseline else {
        current
            .assessment_reasons
            .push("No independent Pro 5x baseline imported".into());
        return;
    };
    current.baseline_source = Some(b.source.clone());
    current.baseline_capacity = b.evidence.summary.capacity_per_week;
    let Some(c) = current.controls.as_ref() else {
        current
            .assessment_reasons
            .push("Start a controlled observation before comparison".into());
        return;
    };
    let Some(bc) = b.evidence.controls.as_ref() else {
        return;
    };
    if c.model != bc.model
        || c.effort != bc.effort
        || c.context_band != bc.context_band
        || c.workload_class != bc.workload_class
    {
        current.assessment_reasons.push(
            "Baseline model, reasoning, context band or workload class does not match".into(),
        );
    }
    if current.origin_id == b.evidence.origin_id {
        current
            .assessment_reasons
            .push("Baseline must come from an independent account origin".into());
    }
    if current.pricing.fingerprint != b.evidence.pricing.fingerprint {
        current
            .assessment_reasons
            .push("Baseline uses a different frozen price catalog".into());
    }
    if current.generated_at - b.evidence.generated_at > 30 * 86400
        || b.evidence.generated_at > current.generated_at + 300
    {
        current
            .assessment_reasons
            .push("Baseline is future-dated or older than 30 days".into());
    }
    if current.summary.quality == "INSUFFICIENT" || b.evidence.summary.quality == "INSUFFICIENT" {
        current
            .assessment_reasons
            .push("Both datasets must meet sample, duration, quota and stability gates".into());
    }
    let x = &current.summary;
    let y = &b.evidence.summary;
    if x.responses > 0
        && y.responses > 0
        && x.tokens.raw_input_tokens > 0
        && y.tokens.raw_input_tokens > 0
    {
        let context = x.tokens.raw_input_tokens as f64 / x.responses as f64;
        let base_context = y.tokens.raw_input_tokens as f64 / y.responses as f64;
        let cache = x.tokens.cached_input_tokens as f64 / x.tokens.raw_input_tokens as f64;
        let base_cache = y.tokens.cached_input_tokens as f64 / y.tokens.raw_input_tokens as f64;
        let output = x.tokens.output_tokens as f64 / x.tokens.raw_input_tokens as f64;
        let base_output = y.tokens.output_tokens as f64 / y.tokens.raw_input_tokens as f64;
        let tools = x.tool_calls as f64 / x.responses as f64;
        let base_tools = y.tool_calls as f64 / y.responses as f64;
        if !(0.75..=1.25).contains(&(context / base_context))
            || (cache - base_cache).abs() > 0.10
            || (output - base_output).abs() > 0.05
            || (tools - base_tools).abs() > 0.25_f64.max(base_tools * 0.25)
        {
            current
                .assessment_reasons
                .push("Baseline context, cache, output or tool mix is not comparable".into());
        }
    }
    if !current.assessment_reasons.is_empty() {
        return;
    }
    if let (Some(x), Some(y), Some(xr), Some(yr)) = (
        x.capacity_per_week,
        y.capacity_per_week,
        x.capacity_range,
        y.capacity_range,
    ) {
        if y <= 0.0 || yr[0] <= 0.0 {
            current
                .assessment_reasons
                .push("Baseline capacity range includes zero".into());
            return;
        }
        current.relative_index = Some(x / y);
        let range = [xr[0] / yr[1], xr[1] / yr[0]];
        current.relative_range = Some(range);
        current.assessment = if range[0] >= 0.65 && range[1] <= 1.5 {
            "PRO-5X-LIKE"
        } else if range[0] >= 2.8 && range[1] <= 5.6 {
            "PRO-20X-LIKE"
        } else {
            "INCONCLUSIVE"
        }
        .into();
        current.confidence =
            if current.summary.quality == "HIGH" && b.evidence.summary.quality == "HIGH" {
                "HIGH"
            } else {
                "MEDIUM"
            }
            .into();
        if current.assessment == "INCONCLUSIVE" {
            current
                .assessment_reasons
                .push("Conservative comparison range does not fit either resemblance band".into());
        }
    }
}

pub fn report(
    store: &Store,
    c: &Catalog,
    settings: &Settings,
    current: &Quota,
    now: i64,
) -> Result<View> {
    let run = latest_run(store, &current.account_key)?;
    let pricing = run
        .as_ref()
        .map(|r| r.config.pricing.clone())
        .unwrap_or_else(|| FrozenPricing::capture(c, settings));
    let start = run.as_ref().map(|r| r.start).unwrap_or(now - 7 * 86400);
    let end = run.as_ref().and_then(|r| r.end).unwrap_or(now);
    let mut stmt=store.conn.prepare("SELECT payload FROM quota_snapshots WHERE timestamp>=? AND timestamp<=? ORDER BY timestamp,account_key")?;
    let texts = stmt
        .query_map(params![start, end], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let history = texts
        .iter()
        .map(|s| serde_json::from_str::<Quota>(s))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let mut rows = vec![];
    let mut anchor: Option<&Quota> = None;
    let mut previous: Option<&Quota> = None;
    let mut flags = vec![];
    let mut ids = BTreeMap::new();
    let max_gap = run.as_ref().map(|r| r.config.max_poll_gap).unwrap_or(300);
    for q in &history {
        if let Some(p) = previous {
            if q.meta.updated_at.unwrap_or(0) - p.meta.updated_at.unwrap_or(0) > max_gap {
                push_unique(&mut flags, "Official polling gap");
            }
            if p.account_key != q.account_key {
                push_unique(&mut flags, "Account changed");
            }
            if p.account_key == q.account_key
                && p.reset_credits
                    .as_ref()
                    .zip(q.reset_credits.as_ref())
                    .is_some_and(|(a, b)| b.available_count < a.available_count)
            {
                push_unique(&mut flags, "Reset credit consumed");
            }
        }
        if q.account_key != current.account_key {
            anchor = None;
            previous = Some(q);
            continue;
        }
        if let Some(a) = anchor {
            let change = match (a.weekly.as_ref(), q.weekly.as_ref()) {
                (Some(a), Some(b)) => {
                    a.used_percent != b.used_percent || a.resets_at != b.resets_at
                }
                _ => true,
            };
            let mut boundary = false;
            if a.plan != q.plan {
                push_unique(&mut flags, "Reported plan changed");
                boundary = true;
            }
            if a.meta.status != "LIVE" || q.meta.status != "LIVE" {
                push_unique(&mut flags, "Non-live official snapshot");
                boundary = true;
            }
            match (a.weekly.as_ref(), q.weekly.as_ref()) {
                (Some(a), Some(b))
                    if a.window_minutes == Some(10080)
                        && b.window_minutes == Some(10080)
                        && a.resets_at.is_some()
                        && a.resets_at == b.resets_at
                        && a.resets_at
                            .is_some_and(|t| t > q.meta.updated_at.unwrap_or(i64::MAX)) =>
                {
                    if b.used_percent < a.used_percent {
                        push_unique(&mut flags, "Quota decreased within a reset window");
                        boundary = true;
                    }
                }
                _ => {
                    push_unique(&mut flags, "Weekly reset or missing window");
                    boundary = true;
                }
            }
            if a.reset_credits
                .as_ref()
                .zip(q.reset_credits.as_ref())
                .is_some_and(|(a, b)| b.available_count < a.available_count)
            {
                push_unique(&mut flags, "Reset credit consumed");
                boundary = true;
            }
            if change || boundary {
                rows.push(interval(
                    store,
                    a,
                    q,
                    &pricing,
                    run.as_ref(),
                    std::mem::take(&mut flags),
                    now,
                    &mut ids,
                )?);
                anchor = Some(q);
            }
        } else {
            anchor = Some(q);
        }
        previous = Some(q);
    }
    if let (Some(a), Some(b)) = (anchor, previous) {
        if b.account_key == current.account_key && b.meta.updated_at > a.meta.updated_at {
            push_unique(&mut flags, "Pending next quota change");
            rows.push(interval(
                store,
                a,
                b,
                &pricing,
                run.as_ref(),
                flags,
                now,
                &mut ids,
            )?);
        }
    }
    if run
        .as_ref()
        .is_some_and(|r| parser_errors(store).is_ok_and(|n| n > r.config.initial_errors))
    {
        for row in &mut rows {
            row.eligible = false;
            push_unique(
                &mut row.reasons,
                "New local parser errors during observation",
            );
        }
    }
    let summary = summarize(&rows);
    let mut evidence=Evidence {method:METHOD.into(),generated_at:now,origin_id:origin(store,&current.account_key)?,reported_plan:plan(current.plan.as_deref()),controls:run.as_ref().map(|r|r.config.controls.clone()),started_at:run.as_ref().map(|r|r.start),ended_at:run.as_ref().and_then(|r|r.end),pricing,intervals:rows,summary,assessment:"INCONCLUSIVE".into(),confidence:"INSUFFICIENT".into(),relative_index:None,relative_range:None,baseline_capacity:None,baseline_source:None,assessment_reasons:vec![],boundary:BOUNDARY.into(),methodology:vec![
        "Each row joins observed official weekly changes to deduplicated local events in (start, end]; unchanged polls are carried into the next change, not discarded. Workloads are distinct session/turn pairs, not poll counts.".into(),
        "10-minute settling delay; reset, saturation, polling-gap, account-switch, unknown-price and protocol-violation rows are retained with exclusions. Polling cannot see every server-side change or all account activity.".into(),
        "USD = (fresh input × input rate + cached input × cached rate + output × output rate) / 1M. Prices are frozen at observation start. Reasoning is already included in output. Service-tier/long-context adjustments are not reconstructed.".into(),
        "Capacity = 100 × sum(eligible API equivalent) / sum(eligible percentage points). Dispersion is quota-weighted SD of window rates. Capacity range combines ±2 SD with assumed ±1 pp per quota endpoint; adjacent eligible endpoints cancel. This is a conservative sensitivity range, not a statistical 95% confidence interval.".into(),
        "MEDIUM requires ≥12 change windows, ≥20 distinct workloads, ≥8h eligible coverage, ≥20pp, 100% priced eligible tokens, and CV ≤25%. HIGH additionally requires ≥30 windows, ≥30 workloads, ≥24h, ≥30pp and Fast/subagent controls verified in logs. Quality labels are heuristic, conditional on exclusive-local-use declarations.".into(),
        "Comparison requires an independent, ≤30-day Pro 5x reference with identical model, reasoning, context band, workload class and price fingerprint. Mean input size must be within 25%, cache within 10pp, output/input within 5pp, and tool/response mix within max(0.25, 25%).".into(),
        "Only a full relative sensitivity range inside [0.65,1.50] is PRO-5X-LIKE, or [2.80,5.60] is PRO-20X-LIKE. The 4× Pro 20x reference is a nominal-tier comparison assumption, not an official fixed weekly-dollar guarantee.".into(),
        "Privacy: no account/email, original session/response IDs, paths, prompts or tool payloads. origin_id is a random local pseudonym; workload labels are report-local ordinals. Imported baselines are user-supplied measurements; hashes verify integrity, not authenticity.".into()
    ]};
    let baseline = load_baseline(store)?;
    compare(&mut evidence, baseline.as_ref());
    let current_live = current.meta.status == "LIVE"
        && current
            .meta
            .updated_at
            .is_some_and(|t| (0..=max_gap).contains(&(now - t)));
    if !current_live {
        evidence.assessment = "INCONCLUSIVE".into();
        evidence.confidence = "INSUFFICIENT".into();
        evidence
            .assessment_reasons
            .push("Current official data is stale, cached or unavailable".into());
    }
    let active_elsewhere: u64 = store.conn.query_row(
        "SELECT COUNT(*) FROM audit_runs WHERE ended_at IS NULL AND account_key!=?",
        [&current.account_key],
        |r| r.get(0),
    )?;
    Ok(View {
        evidence,
        running: run
            .as_ref()
            .is_some_and(|r| r.end.is_none() && r.account == current.account_key),
        active_elsewhere: active_elsewhere > 0,
        baseline_loaded: baseline.is_some(),
        quota_status: if !current_live && current.meta.status == "LIVE" {
            "STALE".into()
        } else {
            current.meta.status.clone()
        },
    })
}

fn load_baseline(store: &Store) -> Result<Option<Baseline>> {
    let value: Option<String> = store
        .conn
        .query_row(
            "SELECT value FROM settings WHERE key='audit_baseline'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    value.map(|v| Ok(serde_json::from_str(&v)?)).transpose()
}
pub fn clear_baseline(store: &Store) -> Result<()> {
    store
        .conn
        .execute("DELETE FROM settings WHERE key='audit_baseline'", [])?;
    Ok(())
}
pub fn envelope(e: &Evidence) -> Result<serde_json::Value> {
    let report = serde_json::to_value(e)?;
    let hash = digest(serde_json::to_vec(&report)?);
    Ok(
        serde_json::json!({"schema_version":1,"kind":"codex-tier-audit","sha256":hash,"report":report}),
    )
}
pub fn import_baseline(
    store: &Store,
    text: &str,
    source: &str,
    current_origin: &str,
    now: i64,
) -> Result<()> {
    ensure!(
        text.len() <= 2 * 1024 * 1024,
        "Baseline must be at most 2 MiB"
    );
    ensure!(
        (source == "private-reference" || source.starts_with("https://github.com/")
            || source.starts_with("https://learn.chatgpt.com/"))
            && !source.contains(['?', '#', '@', '\n'])
            && source.len() <= 300,
        "Use private-reference or a public GitHub/official URL without query, fragment or credentials"
    );
    let v: serde_json::Value = serde_json::from_str(text)?;
    ensure!(
        v["schema_version"] == 1 && v["kind"] == "codex-tier-audit",
        "Import a Tier Auditor evidence JSON"
    );
    ensure!(
        v["sha256"].as_str() == Some(&digest(serde_json::to_vec(&v["report"])?)),
        "Evidence checksum mismatch"
    );
    let mut e: Evidence = serde_json::from_value(v["report"].clone())?;
    ensure!(
        e.method == METHOD && e.reported_plan == "ChatGPT Pro",
        "Reference must use this audit method and report ChatGPT Pro"
    );
    let controls = e
        .controls
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Reference needs controlled observation"))?;
    controls.validate()?;
    ensure!(
        controls.expected_tier == "pro_5x",
        "Reference must identify its tier as user-configured Pro 5x"
    );
    ensure!(
        e.origin_id != current_origin
            && e.origin_id.len() == 64
            && e.origin_id.bytes().all(|b| b.is_ascii_hexdigit()),
        "Reference must have an independent account origin"
    );
    ensure!(
        (now - 30 * 86400..=now + 300).contains(&e.generated_at),
        "Reference must be from the last 30 days"
    );
    ensure!(
        e.ended_at.is_some() && e.started_at.is_some_and(|t| t < e.ended_at.unwrap()),
        "Reference observation must be completed"
    );
    ensure!(e.intervals.len() <= 10000, "Reference has too many rows");
    ensure!(e.pricing.rates.iter().all(|(m,p)|safe_label(m)&&p.iter().all(|v|v.is_finite()&&*v>=0.0&&*v<1e6)),"Invalid reference prices");
    ensure!(
        e.pricing.fingerprint == digest(serde_json::to_vec(&e.pricing.rates)?),
        "Price fingerprint mismatch"
    );
    let mut previous_end = 0;
    for row in &e.intervals {
        ensure!(
            row.start >= previous_end
                && row.start < row.end
                && row.end <= e.generated_at
                && row.start >= e.started_at.unwrap()
                && row.end <= e.ended_at.unwrap(),
            "Overlapping, reversed or out-of-observation reference intervals"
        );
        previous_end = row.end;
        let t = &row.tokens;
        ensure!(
            t.cached_input_tokens <= t.raw_input_tokens
                && t.uncached_input_tokens == t.raw_input_tokens - t.cached_input_tokens
                && t.total_tokens
                    == t.raw_input_tokens
                        .checked_add(t.output_tokens)
                        .ok_or_else(|| anyhow::anyhow!("Token overflow"))?
                && t.reasoning_output_tokens <= t.output_tokens,
            "Inconsistent reference tokens"
        );
        if !row.eligible {
            continue;
        }
        ensure!(
            row.reasons.is_empty()
                && row.models == [controls.model.clone()]
                && row.efforts == [controls.effort.clone()]
                && row.responses > 0
                && !row.workloads.is_empty()
                && row.workloads.len() as u64 <= row.responses
                && row
                    .workloads
                    .iter()
                    .all(|w| w.starts_with("workload-") && safe_label(w)),
            "Invalid reference controls or workload count"
        );
        ensure!(
            context_band(row.input_min) == controls.context_band
                && context_band(row.input_max) == controls.context_band
                && row.input_min <= row.input_max,
            "Reference context mismatch"
        );
        ensure!(
            (row.input_min as u128 * row.responses as u128
                ..=row.input_max as u128 * row.responses as u128)
                .contains(&(t.raw_input_tokens as u128)),
            "Reference context extrema do not contain its input counts"
        );
        ensure!(
            controls.workload_class != "no_tools" || row.tool_calls == 0,
            "Reference violates no-tools protocol"
        );
        ensure!(
            ["LOG_VERIFIED", "USER_ATTESTED"].contains(&row.fast_evidence.as_str())
                && ["LOG_VERIFIED", "USER_ATTESTED"].contains(&row.subagent_evidence.as_str()),
            "Reference Fast/subagent evidence missing"
        );
        ensure!(
            row.unpriced_tokens == 0
                && row.reset_at.is_some_and(|r| r > row.end)
                && e.generated_at - row.end >= SETTLING_SECONDS,
            "Incomplete or unsettled reference"
        );
        let before = row
            .weekly_before
            .ok_or_else(|| anyhow::anyhow!("Reference missing quota"))?;
        let after = row
            .weekly_after
            .ok_or_else(|| anyhow::anyhow!("Reference missing quota"))?;
        let delta = before - after;
        ensure!(
            before <= 100.0
                && after > 1.0
                && delta > 0.0
                && row.delta_quota.is_some_and(|d| (d - delta).abs() < 1e-6),
            "Inconsistent reference quota delta"
        );
        let value = e
            .pricing
            .value(&controls.model, t)
            .ok_or_else(|| anyhow::anyhow!("Reference model unpriced"))?;
        ensure!(
            value.is_finite()
                && (value - row.api_equivalent).abs() < 1e-6_f64.max(value * 1e-9)
                && row
                    .usd_per_percent
                    .is_some_and(|p| (p - value / delta).abs() < 1e-6),
            "Reference dollar calculation mismatch"
        );
    }
    e.summary = summarize(&e.intervals);
    ensure!(
        e.summary.quality != "INSUFFICIENT",
        "Reference fails sample, duration, coverage or stability gates"
    );
    let baseline = Baseline {
        source: source.into(),
        evidence: e,
    };
    store.conn.execute("INSERT INTO settings(key,value) VALUES('audit_baseline',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&baseline)?])?;
    Ok(())
}

pub fn render(e: &Evidence, format: &str) -> Result<String> {
    use crate::export::{csv_cell, escape};
    match format {
        "json" => Ok(serde_json::to_string_pretty(&envelope(e)?)?),
        "csv" => {
            let mut out=String::from("start_utc,end_utc,weekly_remaining_before,weekly_remaining_after,delta_used_pp,fresh_input,cached_input,output,reasoning_in_output,api_equivalent_usd,usd_per_1_percent,model,reasoning_effort,eligible,reasons\n");
            for r in &e.intervals {
                let cells = vec![
                    utc(r.start),
                    utc(r.end),
                    r.weekly_before.map(|v| v.to_string()).unwrap_or_default(),
                    r.weekly_after.map(|v| v.to_string()).unwrap_or_default(),
                    r.delta_quota.map(|v| v.to_string()).unwrap_or_default(),
                    r.tokens.uncached_input_tokens.to_string(),
                    r.tokens.cached_input_tokens.to_string(),
                    r.tokens.output_tokens.to_string(),
                    r.tokens.reasoning_output_tokens.to_string(),
                    r.api_equivalent.to_string(),
                    r.usd_per_percent.map(|v| v.to_string()).unwrap_or_default(),
                    r.models.join(" / "),
                    r.efforts.join(" / "),
                    r.eligible.to_string(),
                    r.reasons.join("; "),
                ];
                out.push_str(
                    &cells
                        .iter()
                        .map(|s| csv_cell(s))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.push('\n');
            }
            Ok(out)
        }
        "html" => {
            let summary = serde_json::to_string_pretty(&e.summary)?;
            let mut out=format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>Pro Tier Auditor — Evidence Report</title><style>body{{font:15px -apple-system,sans-serif;color:#183b34;max-width:1100px;margin:40px auto;padding:24px;line-height:1.6}}table{{border-collapse:collapse;width:100%;font-size:12px}}th,td{{padding:8px;text-align:left;border-bottom:1px solid #dfe6de;vertical-align:top}}pre{{white-space:pre-wrap;background:#f6f7f2;padding:16px}}.scroll{{overflow:auto}}small{{color:#65776e}}</style><h1>Pro Tier Auditor / 套餐额度审计</h1><p>{} · {} · {}</p><h2>{}</h2><p>Confidence: {} (heuristic, conditional)</p><p>{}</p><h2>Observed data</h2><pre>{}</pre><h2>Method and limitations</h2><ol>",escape(&e.reported_plan),escape(&utc(e.generated_at)),escape(METHOD),escape(&e.assessment),escape(&e.confidence),escape(BOUNDARY),escape(&summary));
            for line in &e.methodology {
                out.push_str(&format!("<li>{}</li>", escape(line)));
            }
            out.push_str("</ol><h2>Configuration and comparison</h2><pre>");
            out.push_str(&escape(&serde_json::to_string_pretty(&serde_json::json!({"controls":e.controls,"pricing":e.pricing,"assessment_reasons":e.assessment_reasons,"baseline_source":e.baseline_source,"baseline_capacity":e.baseline_capacity,"relative_index":e.relative_index,"relative_range":e.relative_range}))?));
            out.push_str("</pre><h2>Every observed quota change</h2><div class=\"scroll\"><table><thead><tr><th>UTC interval</th><th>Remaining</th><th>Δ used</th><th>Fresh / cached / output</th><th>API eq.</th><th>$ / 1%</th><th>Model / reasoning</th><th>Eligibility</th></tr></thead><tbody>");
            for r in &e.intervals {
                out.push_str(&format!("<tr><td>{}<br>{}</td><td>{} → {}</td><td>{}</td><td>{} / {} / {}</td><td>${:.4}</td><td>{}</td><td>{}<br>{}</td><td>{}</td></tr>",escape(&utc(r.start)),escape(&utc(r.end)),r.weekly_before.map(|v|format!("{v:.1}%")).unwrap_or("—".into()),r.weekly_after.map(|v|format!("{v:.1}%")).unwrap_or("—".into()),r.delta_quota.map(|v|format!("{v:+.1} pp")).unwrap_or("—".into()),r.tokens.uncached_input_tokens,r.tokens.cached_input_tokens,r.tokens.output_tokens,r.api_equivalent,r.usd_per_percent.map(|v|format!("${v:.4}")).unwrap_or("—".into()),escape(&r.models.join(" / ")),escape(&r.efforts.join(" / ")),escape(&if r.eligible {"Eligible".into()}else{r.reasons.join("; ")})));
            }
            out.push_str("</tbody></table></div><p><small>Share the companion JSON for machine-readable counters, controls, reset times, frozen prices and integrity checksum. No account identifiers or private project content are included.</small></p></html>");
            Ok(out)
        }
        _ => anyhow::bail!("Use json, html or csv"),
    }
}
fn utc(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| "Invalid time".into())
}

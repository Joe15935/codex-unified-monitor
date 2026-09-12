//! Single-pass Rust port of YUHAO's daily/model bucket aggregation.
use crate::{
    account::{Provenance, Quota, Window},
    accounting::Tokens,
    parser::Event,
    pricing::Catalog,
    settings::Settings,
    storage::Store,
};
use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValueTotal {
    pub known_value_usd: f64,
    pub priced_tokens: u64,
    pub unpriced_tokens: u64,
    pub cache_savings_usd: f64,
}
impl ValueTotal {
    pub fn value(&self) -> Option<f64> {
        (self.priced_tokens > 0 || self.unpriced_tokens == 0).then_some(self.known_value_usd)
    }
    pub fn coverage(&self) -> Option<f64> {
        let n = self.priced_tokens + self.unpriced_tokens;
        (n > 0).then(|| self.priced_tokens as f64 / n as f64)
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Aggregate {
    pub tokens: Tokens,
    pub public_api: ValueTotal,
    pub codex_work: ValueTotal,
    pub responses: u64,
    pub tool_calls: u64,
}
impl Aggregate {
    pub fn add(&mut self, e: &Event, c: &Catalog, s: &Settings) {
        self.tokens.add(&e.tokens);
        if e.kind == "tool" {
            self.tool_calls += 1;
        }
        if e.kind != "token" {
            return;
        }
        self.responses += 1;
        for mode in ["public_api", "codex_work"] {
            let target = if mode == "public_api" {
                &mut self.public_api
            } else {
                &mut self.codex_work
            };
            if let Some(p) = c.resolve(&e.model, mode, s) {
                target.priced_tokens += e.tokens.total_tokens;
                target.known_value_usd += p.value(&e.tokens);
                target.cache_savings_usd += p.savings(&e.tokens);
            } else {
                target.unpriced_tokens += e.tokens.total_tokens;
            }
        }
    }
    pub fn money(&self, mode: &str) -> &ValueTotal {
        if mode == "codex_work" {
            &self.codex_work
        } else {
            &self.public_api
        }
    }
    fn accumulate(&mut self, event_value: &Self) {
        self.tokens.add(&event_value.tokens);
        self.responses += event_value.responses;
        self.tool_calls += event_value.tool_calls;
        for (target, value) in [
            (&mut self.public_api, &event_value.public_api),
            (&mut self.codex_work, &event_value.codex_work),
        ] {
            target.known_value_usd += value.known_value_usd;
            target.priced_tokens += value.priced_tokens;
            target.unpriced_tokens += value.unpriced_tokens;
            target.cache_savings_usd += value.cache_savings_usd;
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub period: String,
    pub start: Option<String>,
    pub end: Option<String>,
}
impl Default for Range {
    fn default() -> Self {
        Self {
            period: "today".into(),
            start: None,
            end: None,
        }
    }
}
pub fn midnight(tz: chrono_tz::Tz, date: NaiveDate) -> Result<i64> {
    let t = date.and_hms_opt(0, 0, 0).unwrap();
    for h in 0..4 {
        if let Some(dt) = tz.from_local_datetime(&(t + Duration::hours(h))).earliest() {
            return Ok(dt.timestamp());
        }
    }
    anyhow::bail!("This calendar date does not exist in the chosen timezone")
}
pub fn bounds(range: &Range, tz: chrono_tz::Tz, now: i64) -> Result<(i64, i64)> {
    let today = tz
        .timestamp_opt(now, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Invalid current time"))?
        .date_naive();
    let start = match range.period.as_str() {
        "today" => midnight(tz, today)?,
        "5h" => now - 18000,
        "week" => midnight(
            tz,
            today - Duration::days(today.weekday().num_days_from_monday() as i64),
        )?,
        "month" => midnight(tz, today.with_day(1).unwrap())?,
        "year" => midnight(tz, NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap())?,
        "30d" => now - 30 * 86400,
        "90d" => now - 90 * 86400,
        "all" => 0,
        "custom" => midnight(
            tz,
            NaiveDate::parse_from_str(range.start.as_deref().unwrap_or(""), "%Y-%m-%d")?,
        )?,
        _ => anyhow::bail!("Unknown time range"),
    };
    let end = if range.period == "custom" {
        let date = NaiveDate::parse_from_str(range.end.as_deref().unwrap_or(""), "%Y-%m-%d")?;
        midnight(
            tz,
            date.succ_opt()
                .ok_or_else(|| anyhow::anyhow!("Invalid end date"))?,
        )?
    } else {
        now + 1
    };
    anyhow::ensure!(end > start, "End date must follow start date");
    Ok((start, end))
}
#[derive(Debug, Clone, Serialize)]
pub struct ModelRow {
    pub model: String,
    pub aggregate: Aggregate,
}
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub timestamp: i64,
    pub label: String,
    pub aggregate: Aggregate,
}
#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub project: String,
    pub started_at: i64,
    pub last_active: i64,
    pub models: BTreeSet<String>,
    pub efforts: BTreeSet<String>,
    pub turns: BTreeSet<String>,
    pub aggregate: Aggregate,
}
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostics {
    pub session_files: u64,
    pub parsed_files: u64,
    pub events: u64,
    pub duplicates_rejected: u64,
    pub parser_errors: u64,
    pub oversized_lines: u64,
    pub last_offset: u64,
    pub database_bytes: u64,
    pub last_local_update: Option<i64>,
    pub scan_status: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub meta: Provenance,
    pub range: Range,
    pub timezone: String,
    pub start: i64,
    pub end: i64,
    pub total: Aggregate,
    pub periods: BTreeMap<String, Aggregate>,
    pub models: Vec<ModelRow>,
    pub sessions: Vec<SessionRow>,
    pub trend: Vec<Point>,
    pub tools: Vec<(String, u64)>,
    pub diagnostics: Diagnostics,
    pub catalog: Catalog,
}
pub fn report(
    store: &Store,
    catalog: &Catalog,
    settings: &Settings,
    range: Range,
    now: i64,
) -> Result<Report> {
    let tz = settings.timezone.parse::<chrono_tz::Tz>()?;
    let (start, end) = bounds(&range, tz, now)?;
    let mut periods = BTreeMap::<String, Aggregate>::new();
    let mut period_bounds = vec![];
    for period in ["today", "5h", "week", "month", "30d", "90d", "year", "all"] {
        periods.insert(period.into(), Aggregate::default());
        let b = bounds(
            &Range {
                period: period.into(),
                ..Range::default()
            },
            tz,
            now,
        )?;
        period_bounds.push((period, b));
    }
    let projects = store
        .conn
        .prepare("SELECT id,project,started_at FROM sessions")?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, i64>(2)?),
            ))
        })?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    let mut total = Aggregate::default();
    let mut models = BTreeMap::<String, Aggregate>::new();
    let mut sessions = BTreeMap::<String, SessionRow>::new();
    let mut trend = BTreeMap::<i64, Point>::new();
    let mut tools = BTreeMap::<String, u64>::new();
    let hourly = matches!(range.period.as_str(), "today" | "5h");
    let scan_end = period_bounds
        .iter()
        .map(|(_, (_, end))| *end)
        .fold(end, i64::max);
    store.visit_events(0, scan_end, |e| {
        // The eight summary periods include all time, so this pass must retain
        // all events. Resolve both prices once per event, then add the exact same
        // contribution to each bucket in the original event order.
        let mut event_value = Aggregate::default();
        event_value.add(&e, catalog, settings);
        for (key, (a, b)) in &period_bounds {
            if e.timestamp >= *a && e.timestamp < *b {
                periods.get_mut(*key).unwrap().accumulate(&event_value);
            }
        }
        if e.timestamp < start || e.timestamp >= end {
            return;
        }
        total.accumulate(&event_value);
        let dt = tz.timestamp_opt(e.timestamp, 0).single().unwrap();
        let bucket = if hourly {
            e.timestamp - dt.minute() as i64 * 60 - dt.second() as i64
        } else {
            midnight(tz, dt.date_naive()).unwrap_or(e.timestamp)
        };
        trend
            .entry(bucket)
            .or_insert_with(|| Point {
                timestamp: bucket,
                label: dt
                    .format(if hourly {
                        "%m-%d %H:00 %:z"
                    } else {
                        "%Y-%m-%d"
                    })
                    .to_string(),
                aggregate: Aggregate::default(),
            })
            .aggregate
            .accumulate(&event_value);
        let session = sessions.entry(e.session_id.clone()).or_insert_with(|| {
            let p = projects.get(&e.session_id);
            SessionRow {
                id: e.session_id.clone(),
                project: p.map(|p| p.0.clone()).unwrap_or_default(),
                started_at: p.map(|p| p.1).filter(|t| *t > 0).unwrap_or(e.timestamp),
                last_active: e.timestamp,
                models: BTreeSet::new(),
                efforts: BTreeSet::new(),
                turns: BTreeSet::new(),
                aggregate: Aggregate::default(),
            }
        });
        session.last_active = session.last_active.max(e.timestamp);
        session.aggregate.accumulate(&event_value);
        if !e.turn_id.is_empty() {
            session.turns.insert(e.turn_id.clone());
        } else if e.kind == "turn" {
            session.turns.insert(e.id.clone());
        }
        if e.kind == "token" {
            session.models.insert(e.model.clone());
            if !e.effort.is_empty() {
                session.efforts.insert(e.effort.clone());
            }
            models
                .entry(e.model.clone())
                .or_default()
                .accumulate(&event_value);
        }
        if e.kind == "tool" {
            *tools.entry(e.tool).or_default() += 1;
        }
    })?;
    let mut model_rows: Vec<_> = models
        .into_iter()
        .map(|(model, aggregate)| ModelRow { model, aggregate })
        .collect();
    model_rows.sort_by_key(|m| std::cmp::Reverse(m.aggregate.tokens.total_tokens));
    let mut session_rows: Vec<_> = sessions.into_values().collect();
    session_rows.sort_by(|a, b| {
        b.aggregate
            .money(&settings.pricing_mode)
            .known_value_usd
            .total_cmp(&a.aggregate.money(&settings.pricing_mode).known_value_usd)
    });
    let mut tool_rows: Vec<_> = tools.into_iter().collect();
    tool_rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    Ok(Report {
        meta: Provenance::new("local Codex rollout metadata", "LOCAL", Some(now)),
        range,
        timezone: settings.timezone.clone(),
        start,
        end,
        total,
        periods,
        models: model_rows,
        sessions: session_rows,
        trend: trend.into_values().collect(),
        tools: tool_rows,
        diagnostics: diagnostics(store)?,
        catalog: catalog.clone(),
    })
}
pub fn diagnostics(store: &Store) -> Result<Diagnostics> {
    let n = |sql: &str| store.conn.query_row(sql, [], |r| r.get::<_, u64>(0));
    Ok(Diagnostics {
        session_files: n("SELECT COUNT(*) FROM files")?,
        parsed_files: n("SELECT COUNT(*) FROM files WHERE offset>0")?,
        events: n("SELECT COUNT(*) FROM events")?,
        duplicates_rejected: n(
            "SELECT COALESCE(SUM(value),0) FROM diagnostics WHERE key='duplicates'",
        )?,
        parser_errors: n("SELECT COALESCE(SUM(errors),0) FROM files")?,
        oversized_lines: n("SELECT COALESCE(SUM(oversized),0) FROM files")?,
        last_offset: n("SELECT COALESCE(MAX(offset),0) FROM files")?,
        database_bytes: std::fs::metadata(&store.path).map(|m| m.len()).unwrap_or(0)
            + std::fs::metadata(format!("{}-wal", store.path.display()))
                .map(|m| m.len())
                .unwrap_or(0),
        last_local_update: None,
        scan_status: "ready".into(),
    })
}
#[derive(Debug, Clone, Serialize)]
pub struct Burn {
    pub status: String,
    pub samples: usize,
    pub observed_seconds: i64,
    pub percent_per_hour: Option<f64>,
    pub percent_per_day: Option<f64>,
    pub exhausted_at: Option<i64>,
    pub resets_before_exhaustion: bool,
    pub tokens_per_percent: Option<f64>,
    pub output_per_percent: Option<f64>,
    pub equivalent_usd_per_percent: Option<f64>,
    pub pricing_coverage: Option<f64>,
}
pub fn burn(
    history: &[Quota],
    which: &str,
    store: &Store,
    catalog: &Catalog,
    settings: &Settings,
) -> Result<Burn> {
    let window = |q: &Quota| -> Option<Window> {
        if which == "5h" {
            q.five_hour.clone()
        } else {
            q.weekly.clone()
        }
    };
    let mut result = Burn {
        status: "Collecting data".into(),
        samples: 0,
        observed_seconds: 0,
        percent_per_hour: None,
        percent_per_day: None,
        exhausted_at: None,
        resets_before_exhaustion: false,
        tokens_per_percent: None,
        output_per_percent: None,
        equivalent_usd_per_percent: None,
        pricing_coverage: None,
    };
    let Some(last) = history.last() else {
        return Ok(result);
    };
    let Some(w) = window(last) else {
        result.status = "Unavailable".into();
        return Ok(result);
    };
    let last_at = last.meta.updated_at.unwrap_or(0);
    let mut segment = vec![];
    let mut previous = w.used_percent;
    for q in history.iter().rev() {
        let Some(qw) = window(q) else { break };
        let t = q.meta.updated_at.unwrap_or(0);
        if q.account_key != last.account_key
            || qw.resets_at != w.resets_at
            || qw.used_percent > previous
            || last_at - t > 18000
        {
            break;
        }
        segment.push((t, qw.used_percent));
        previous = qw.used_percent;
    }
    segment.reverse();
    result.samples = segment.len();
    let Some(first) = segment.first() else {
        return Ok(result);
    };
    let span = last_at - first.0;
    result.observed_seconds = span;
    if segment.len() < 4
        || span < 1800
        || last_at < crate::now() - settings.quota_poll_seconds as i64 * 3
    {
        return Ok(result);
    }
    let used = w.used_percent - first.1;
    if used < 1.0 {
        return Ok(result);
    }
    let rate = used / span as f64;
    result.status = "Observed estimate".into();
    result.percent_per_hour = Some(rate * 3600.0);
    result.percent_per_day = Some(rate * 86400.0);
    let exhaustion = last_at + ((100.0 - w.used_percent) / rate).round() as i64;
    result.exhausted_at = Some(exhaustion);
    result.resets_before_exhaustion = w.resets_at.is_some_and(|r| r < exhaustion);
    let mut agg = Aggregate::default();
    store.visit_events(first.0, last_at + 1, |e| agg.add(&e, catalog, settings))?;
    result.tokens_per_percent = Some(agg.tokens.total_tokens as f64 / used);
    result.output_per_percent = Some(agg.tokens.output_tokens as f64 / used);
    result.equivalent_usd_per_percent = agg.money(&settings.pricing_mode).value().map(|v| v / used);
    result.pricing_coverage = agg.money(&settings.pricing_mode).coverage();
    Ok(result)
}
#[derive(Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub timeline: Vec<Point>,
    pub model_changes: Vec<(i64, String, String)>,
    pub tools: Vec<(String, u64)>,
    pub aggregate: Aggregate,
}
pub fn session_detail(store: &Store, id: &str, c: &Catalog, s: &Settings) -> Result<SessionDetail> {
    let tz = s.timezone.parse::<chrono_tz::Tz>()?;
    let mut timeline = BTreeMap::<i64, Point>::new();
    let mut changes = vec![];
    let mut last = String::new();
    let mut tools = BTreeMap::<String, u64>::new();
    let mut aggregate = Aggregate::default();
    store.visit_session_events(id, |e| {
        aggregate.add(&e, c, s);
        let hour = e.timestamp / 3600 * 3600;
        timeline
            .entry(hour)
            .or_insert_with(|| Point {
                timestamp: hour,
                label: tz
                    .timestamp_opt(hour, 0)
                    .single()
                    .unwrap()
                    .format("%m-%d %H:%M")
                    .to_string(),
                aggregate: Aggregate::default(),
            })
            .aggregate
            .add(&e, c, s);
        if e.kind == "tool" {
            *tools.entry(e.tool.clone()).or_default() += 1;
        }
        if e.kind == "token" {
            let key = format!("{}:{}", e.model, e.effort);
            if key != last {
                last = key;
                changes.push((e.timestamp, e.model, e.effort));
            }
        }
    })?;
    Ok(SessionDetail {
        id: id.into(),
        timeline: timeline.into_values().collect(),
        model_changes: changes,
        tools: tools.into_iter().collect(),
        aggregate,
    })
}

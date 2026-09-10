use chrono::{NaiveDate, TimeZone};
use codexmeter_core::{
    account::{normalize, Provenance},
    analytics::{bounds, burn, midnight, Aggregate, Range},
    export::{csv_cell, escape},
    pricing::Catalog,
    settings::Settings,
    storage::Store,
};
use serde_json::json;
#[test]
fn timezone_calendar_week_starts_monday() {
    let tz = chrono_tz::America::New_York;
    let now = tz
        .with_ymd_and_hms(2026, 9, 10, 12, 0, 0)
        .unwrap()
        .timestamp();
    let (start, _) = bounds(
        &Range {
            period: "week".into(),
            ..Range::default()
        },
        tz,
        now,
    )
    .unwrap();
    assert_eq!(
        tz.timestamp_opt(start, 0)
            .unwrap()
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        "2026-09-07 00:00"
    );
}
#[test]
fn dst_spring_day_is_23_hours() {
    let tz = chrono_tz::America::New_York;
    assert_eq!(
        midnight(tz, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()).unwrap()
            - midnight(tz, NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()).unwrap(),
        23 * 3600
    );
}
#[test]
fn dst_fall_day_is_25_hours() {
    let tz = chrono_tz::America::New_York;
    assert_eq!(
        midnight(tz, NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()).unwrap()
            - midnight(tz, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()).unwrap(),
        25 * 3600
    );
}
#[test]
fn custom_end_day_is_inclusive() {
    let tz = chrono_tz::UTC;
    let (a, b) = bounds(
        &Range {
            period: "custom".into(),
            start: Some("2026-09-09".into()),
            end: Some("2026-09-10".into()),
        },
        tz,
        1,
    )
    .unwrap();
    assert_eq!(b - a, 172800);
}
#[test]
fn unknown_not_silently_priced_at_zero() {
    let mut agg = Aggregate::default();
    agg.public_api.unpriced_tokens = 100;
    assert_eq!(agg.public_api.value(), None);
    assert_eq!(agg.public_api.coverage(), Some(0.0));
}
#[test]
fn exports_escape_html_and_spreadsheet_formulas() {
    assert_eq!(escape("<script>\"'&"), "&lt;script&gt;&quot;&#39;&amp;");
    assert_eq!(csv_cell("=SUM(A1)"), "\"'=SUM(A1)\"");
    assert_eq!(csv_cell("a,b\"c"), "\"a,b\"\"c\"");
}
#[test]
fn no_forecast_from_one_point_or_reset_crossing() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(&d.path().join("db")).unwrap();
    let now = codexmeter_core::now();
    let mut samples = vec![];
    for i in 0..4 {
        let mut q=normalize(&json!({"rateLimits":{"primary":{"usedPercent":20+i,"windowDurationMins":300,"resetsAt":now+7200}}}),now-1800+i*600).unwrap();
        q.account_key = "synthetic".into();
        q.meta = Provenance::new("synthetic", "LIVE", Some(now - 1800 + i * 600));
        samples.push(q);
    }
    let s = Settings::default();
    let c = Catalog::bundled();
    assert!(burn(&samples[..1], "5h", &store, &c, &s)
        .unwrap()
        .exhausted_at
        .is_none());
    let b = burn(&samples, "5h", &store, &c, &s).unwrap();
    assert_eq!(b.percent_per_hour, Some(6.0));
    assert!(b.resets_before_exhaustion);
    samples[3].five_hour.as_mut().unwrap().resets_at = Some(now + 8000);
    assert!(burn(&samples, "5h", &store, &c, &s)
        .unwrap()
        .exhausted_at
        .is_none());
}
#[test]
fn cached_restart_never_live() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(&d.path().join("db")).unwrap();
    let mut q = normalize(&json!({"rateLimits":{}}), 123).unwrap();
    q.account_key = "synthetic".into();
    codexmeter_core::account::save(&store, &q).unwrap();
    assert_eq!(
        codexmeter_core::account::cached(&store)
            .unwrap()
            .meta
            .status,
        "CACHED"
    );
}

#[test]
fn quota_csv_uses_quota_columns_and_html_is_a_local_table() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(&d.path().join("db")).unwrap();
    let report = codexmeter_core::analytics::report(
        &store,
        &Catalog::bundled(),
        &Settings::default(),
        Range::default(),
        codexmeter_core::now(),
    )
    .unwrap();
    let q=normalize(&json!({"rateLimits":{"primary":{"usedPercent":25,"windowDurationMins":10080,"resetsAt":1900000000}}}),123).unwrap();
    let csv = codexmeter_core::export::render(&report, &[q.clone()], "csv", "quota").unwrap();
    assert!(csv.starts_with("timestamp,source,status,5h_used_percent"));
    assert!(csv.contains("\"25.0\",\"75.0\""));
    assert!(!csv.contains("raw_input"));
    let html = codexmeter_core::export::render(&report, &[q], "html", "all").unwrap();
    assert!(html.contains("<table>"));
    assert!(!html.contains("<script") && !html.contains("<link"));
}

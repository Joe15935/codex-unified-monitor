use chrono::TimeZone;
use codexmeter_core::{
    analytics::{self, Aggregate, Range},
    pricing::Catalog,
    settings::Settings,
    storage::Store,
};
use rusqlite::params;

fn insert(store: &Store, id: &str, session: &str, time: i64, model: &str, kind: &str) {
    let (raw, cached, output) = if kind == "token" {
        (101, 73, 19)
    } else {
        (0, 0, 0)
    };
    store.conn.execute("INSERT INTO events(id,session_id,timestamp,model,effort,turn_id,kind,tool,raw_input,cached_input,output,reasoning,cache_write,source) VALUES(?,?,?,?,'high','turn',?,'synthetic-tool',?,?,?,0,0,'synthetic')", params![id,session,time,model,kind,raw,cached,output]).unwrap();
}

#[test]
fn indexed_session_filter_preserves_tie_order_and_ignores_unrelated_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("db")).unwrap();
    insert(&store, "z", "selected", 300, "gpt-6-astra", "token");
    insert(&store, "b", "selected", 100, "gpt-6-astra", "token");
    insert(&store, "a", "selected", 100, "gpt-6-astra", "tool");
    insert(&store, "zero", "selected", 0, "gpt-6-astra", "token");
    insert(&store, "negative", "selected", -1, "unknown", "token");
    insert(
        &store,
        "max-timestamp",
        "selected",
        i64::MAX,
        "unknown",
        "token",
    );
    for i in 0..100 {
        insert(
            &store,
            &format!("unrelated-{i}"),
            "unrelated",
            i,
            "unknown",
            "token",
        );
    }
    let mut ids = Vec::new();
    store
        .visit_session_events("selected", |e| ids.push(e.id))
        .unwrap();
    assert_eq!(
        ids,
        ["zero", "a", "b", "z"],
        "preserve the former full-scan [0, i64::MAX) timestamp bounds"
    );
    let mut visits = 0;
    store
        .visit_session_events("selected' OR 1=1 --", |_| visits += 1)
        .unwrap();
    assert_eq!(
        visits, 0,
        "session identifiers must stay bound SQL parameters"
    );
    let settings = Settings {
        timezone: "UTC".into(),
        ..Settings::default()
    };
    let catalog = Catalog::bundled();
    let detail = analytics::session_detail(&store, "selected", &catalog, &settings).unwrap();
    let mut expected = Aggregate::default();
    store
        .visit_events(0, i64::MAX, |e| {
            if e.session_id == "selected" {
                expected.add(&e, &catalog, &settings);
            }
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(detail.aggregate).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(detail.tools, [("synthetic-tool".to_owned(), 1)]);
    assert_eq!(detail.model_changes.len(), 1);
}

#[test]
fn all_summary_periods_keep_exact_event_arithmetic_with_aliases_and_unknown_prices() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("db")).unwrap();
    let now = chrono_tz::America::New_York
        .with_ymd_and_hms(2026, 9, 12, 12, 0, 0)
        .unwrap()
        .timestamp();
    let mut settings = Settings {
        timezone: "America/New_York".into(),
        ..Settings::default()
    };
    settings
        .aliases
        .insert("synthetic-alias".into(), "gpt-6-astra".into());
    let catalog = Catalog::bundled();
    let mut custom = catalog
        .resolve("gpt-6-astra", "public_api", &settings)
        .unwrap()
        .clone();
    custom.input = 1.23456789;
    custom.cached_input = 0.7654321;
    settings.custom_rates.push(custom);
    for (i, age) in [
        1,
        18000,
        86400,
        7 * 86400,
        31 * 86400,
        100 * 86400,
        400 * 86400,
    ]
    .into_iter()
    .enumerate()
    {
        for (j, model) in ["synthetic-alias", "unknown-model", "gpt-6-astra"]
            .into_iter()
            .enumerate()
        {
            insert(
                &store,
                &format!("token-{i}-{j}"),
                "selected",
                now - age,
                model,
                "token",
            );
        }
        insert(
            &store,
            &format!("tool-{i}"),
            "selected",
            now - age,
            "",
            "tool",
        );
    }
    insert(
        &store,
        "future-token",
        "selected",
        now + 86400,
        "gpt-6-astra",
        "token",
    );
    let events = store.events(0, i64::MAX).unwrap();
    for period in ["today", "5h", "week", "month", "30d", "90d", "year", "all"] {
        let range = Range {
            period: period.into(),
            ..Range::default()
        };
        let report = analytics::report(&store, &catalog, &settings, range, now).unwrap();
        let expected = |start, end| {
            let mut value = Aggregate::default();
            for event in &events {
                if event.timestamp >= start && event.timestamp < end {
                    value.add(event, &catalog, &settings);
                }
            }
            serde_json::to_value(value).unwrap()
        };
        assert_eq!(
            serde_json::to_value(&report.total).unwrap(),
            expected(report.start, report.end)
        );
        for (key, value) in &report.periods {
            let (start, end) = analytics::bounds(
                &Range {
                    period: key.clone(),
                    ..Range::default()
                },
                chrono_tz::America::New_York,
                now,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                expected(start, end),
                "summary {key} while viewing {period}"
            );
        }
        assert!(
            report.periods["all"].tokens.total_tokens > report.periods["today"].tokens.total_tokens
        );
        assert!(report.total.public_api.unpriced_tokens > 0);
        assert_eq!(
            serde_json::to_value(&report.sessions[0].aggregate).unwrap(),
            serde_json::to_value(&report.total).unwrap()
        );
    }
    let future = analytics::report(
        &store,
        &catalog,
        &settings,
        Range {
            period: "custom".into(),
            start: Some("2026-09-13".into()),
            end: Some("2026-09-13".into()),
        },
        now,
    )
    .unwrap();
    assert_eq!(
        future.total.responses, 1,
        "a custom future range extends the scan beyond current summary periods"
    );
    assert_eq!(
        future.periods["all"].responses, 21,
        "current summary periods still exclude future timestamps"
    );
}

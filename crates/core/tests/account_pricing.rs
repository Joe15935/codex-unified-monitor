use codexmeter_core::{
    account::{backoff, normalize, timestamp, READ_METHODS},
    accounting::Tokens,
    pricing::Catalog,
    settings::Settings,
};
use serde_json::json;
#[test]
fn weekly_primary_is_not_five_hour() {
    let q=normalize(&json!({"rateLimits":{"primary":{"usedPercent":33,"windowDurationMins":10080,"resetsAt":1789435505}}}),10).unwrap();
    assert!(q.five_hour.is_none());
    assert_eq!(q.weekly.unwrap().remaining_percent, 67.0);
}
#[test]
fn null_window_and_reset_credit_fields_are_unavailable() {
    let q = normalize(
        &json!({"rateLimits":{"primary":null},"rateLimitResetCredits":null}),
        10,
    )
    .unwrap();
    assert!(q.weekly.is_none() && q.five_hour.is_none() && q.reset_credits.is_none());
}
#[test]
fn missing_used_percent_not_zero() {
    let q = normalize(
        &json!({"rateLimits":{"primary":{"windowDurationMins":300}}}),
        10,
    )
    .unwrap();
    assert!(q.five_hour.is_none());
}
#[test]
fn quota_selects_codex_not_spark() {
    let q=normalize(&json!({"rateLimits":{"primary":{"usedPercent":99,"windowDurationMins":300}},"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":20,"windowDurationMins":300}},"spark":{"primary":{"usedPercent":3,"windowDurationMins":300}}}}),10).unwrap();
    assert_eq!(q.five_hour.unwrap().used_percent, 20.0);
    assert_eq!(q.buckets.len(), 2);
}
#[test]
fn reset_detail_null_differs_from_empty() {
    for (detail, expected) in [(json!(null), false), (json!([]), true)] {
        let q = normalize(
            &json!({"rateLimits":{},"rateLimitResetCredits":{"availableCount":2,"credits":detail}}),
            10,
        )
        .unwrap();
        let r = q.reset_credits.unwrap();
        assert_eq!(r.available_count, 2);
        assert_eq!(r.details.is_some(), expected);
    }
}
#[test]
fn reset_seconds_and_milliseconds() {
    assert_eq!(timestamp(&json!(1789435505000i64)), Some(1789435505));
    assert_eq!(timestamp(&json!(1789435505)), Some(1789435505));
    assert_eq!(timestamp(&json!(null)), None);
}
#[test]
fn exponential_backoff_is_bounded() {
    assert_eq!(
        (0..5).map(|n| backoff(90, n)).collect::<Vec<_>>(),
        vec![90, 180, 360, 720, 1440]
    );
    assert_eq!(backoff(90, 99), 3600);
}
#[test]
fn no_account_writes_allowlisted() {
    for m in [
        "account/logout",
        "account/login/start",
        "account/rateLimitResetCredit/consume",
        "thread/start",
        "turn/start",
    ] {
        assert!(!READ_METHODS.contains(&m));
    }
}
#[test]
fn three_manual_costs_and_cache_savings() {
    let c = Catalog::bundled();
    let s = Settings::default();
    let t = Tokens::new(1_000_000, 600_000, 100_000, 50_000, 0).unwrap();
    for (model, expected, saved) in [
        ("gpt-5.6-sol", 3.84, 2.16),
        ("gpt-5.6-terra", 2.12, 1.08),
        ("gpt-6-astra", 9.6, 5.4),
    ] {
        let p = c.resolve(model, "public_api", &s).unwrap();
        assert!((p.value(&t) - expected).abs() < 1e-10);
        assert!((p.savings(&t) - saved).abs() < 1e-10);
    }
}
#[test]
fn unknown_model_has_no_price_or_implicit_alias() {
    let c = Catalog::bundled();
    let mut s = Settings::default();
    assert!(c.resolve("codex-auto-review", "public_api", &s).is_none());
    assert!(c.resolve("gpt-5.3-codex-spark", "public_api", &s).is_none());
    s.aliases.insert("my-model".into(), "gpt-5.6-sol".into());
    assert!(c.resolve("my-model", "public_api", &s).is_some());
}
#[test]
fn catalogs_keep_modes_and_provenance_separate() {
    let c = Catalog::bundled();
    let s = Settings::default();
    let a = c.resolve("gpt-6-astra", "public_api", &s).unwrap();
    let b = c.resolve("gpt-6-astra", "codex_work", &s).unwrap();
    assert_ne!(a.source, b.source);
    assert_ne!(a.cache_write, b.cache_write);
    for p in &c.entries {
        p.validate().unwrap();
        assert_eq!(p.verified_at, "2026-09-10");
    }
}

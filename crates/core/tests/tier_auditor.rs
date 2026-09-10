use codexmeter_core::{
    account::{self, Quota},
    auditor::{self, Baseline, Controls, Evidence},
    ingest,
    parser::{self, ParserState},
    pricing::{Catalog, Price},
    settings::Settings,
    storage::Store,
};
use serde_json::json;
use std::fs;

const START: i64 = 1_800_000_000;
const END: i64 = START + 30 * 3600;
const NOW: i64 = END + 900;
fn time(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).unwrap().to_rfc3339()
}
fn quota(t: i64, used: f64) -> Quota {
    let mut q=account::normalize(&json!({"rateLimits":{"planType":"pro","primary":{"usedPercent":used,"windowDurationMins":10080,"resetsAt":START+604800}}}),t).unwrap();
    q.account_key = "test-account".into();
    q.account_label = Some("private-person@example.invalid".into());
    q.plan = Some("pro".into());
    q
}
fn controls() -> Controls {
    Controls {
        expected_tier: "pro_5x".into(),
        model: "audit-test-model".into(),
        effort: "high".into(),
        context_band: "medium".into(),
        workload_class: "coding".into(),
        fast_off_attested: true,
        no_subagents_attested: true,
        exclusive_local_use_attested: true,
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    store: Store,
    c: Catalog,
    s: Settings,
    q: Quota,
}
impl Fixture {
    fn view(&self) -> auditor::View {
        auditor::report(&self.store, &self.c, &self.s, &self.q, NOW).unwrap()
    }
    fn evidence(&self) -> Evidence {
        self.view().evidence
    }
}
fn fixture(factor: u64, tier: Option<&str>) -> Fixture {
    let d = tempfile::tempdir().unwrap();
    let mut store = Store::open(&d.path().join("db")).unwrap();
    let s = Settings::default();
    let c = Catalog {
        schema_version: 1,
        verified_at: "2027-01-15".into(),
        entries: vec![Price {
            model: "audit-test-model".into(),
            input: 100.0,
            cached_input: 10.0,
            output: 300.0,
            cache_write: None,
            source: "https://developers.openai.com/api/docs/pricing".into(),
            effective_date: None,
            verified_at: "2027-01-15".into(),
            pricing_mode: "public_api".into(),
            processing: "standard".into(),
            context: "base".into(),
            notes: "Synthetic test rates; never shipped as a baseline".into(),
        }],
    };
    let q = quota(START, 5.0);
    auditor::start(&store, &q, &c, &s, controls(), START).unwrap();
    let mut lines = vec![
        json!({"timestamp":time(START),"type":"session_meta","payload":{"id":"private-real-session-id","cwd":"/Users/Private/project-secret","thread_source":"user","source":"vscode"}}),
    ];
    for window in 1..=30 {
        for i in 0..2 * factor {
            let t = START + window * 3600 - 300 + (i as i64) * 3;
            let turn = format!("private-turn-{window}-{i}");
            lines.push(json!({"timestamp":time(t),"type":"turn_context","payload":{"model":"audit-test-model","effort":"high","turn_id":turn,"service_tier":tier}}));
            lines.push(json!({"timestamp":time(t+1),"type":"token_usage_record","payload":{"thread_id":"private-real-session-id","turn_id":turn,"response_id":format!("private-response-{window}-{i}"),"usage":{"input_tokens":100000,"cached_input_tokens":80000,"output_tokens":10000,"reasoning_output_tokens":3000}}}));
        }
    }
    let log = d.path().join("rollout.jsonl");
    fs::write(
        &log,
        lines.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    ingest::ingest(&mut store, &[log]).unwrap();
    for tick in 0..=30 * 12 + 3 {
        let t = START + tick * 300;
        let used = 5.0 + ((tick / 12).min(30) as f64) * 2.0;
        account::save(&store, &quota(t, used)).unwrap();
    }
    Fixture {
        _dir: d,
        store,
        c,
        s,
        q: quota(NOW, 65.0),
    }
}
fn compare_with(mut e: Evidence, b: &Evidence) -> Evidence {
    e.assessment_reasons.clear();
    let baseline = Baseline {
        source: "https://github.com/example/evidence".into(),
        evidence: b.clone(),
    };
    auditor::compare(&mut e, Some(&baseline));
    e
}
#[test]
fn quota_change_joins_unchanged_polls_and_counts_workloads_not_polls() {
    let f = fixture(1, Some("standard"));
    let e = f.evidence();
    let s = e.summary;
    assert_eq!(s.samples, 30);
    assert_eq!(s.workloads, 60);
    assert_eq!(s.observation_seconds, 108000);
    assert_eq!(s.quota_change, 60.0);
    assert_eq!(s.tokens.uncached_input_tokens, 1_200_000);
    assert_eq!(s.tokens.cached_input_tokens, 4_800_000);
    assert_eq!(s.tokens.output_tokens, 600_000);
    // Independently: (1.2M × 100 + 4.8M × 10 + .6M × 300) / 1M = $348.
    assert!((s.api_equivalent - 348.0).abs() < 1e-8);
    assert!((s.usd_per_percent.unwrap() - 5.8).abs() < 1e-8);
    assert!((s.capacity_per_week.unwrap() - 580.0).abs() < 1e-8);
    assert_eq!(s.quality, "HIGH");
    assert_eq!(e.assessment, "INCONCLUSIVE");
    assert_eq!(e.confidence, "INSUFFICIENT");
}
#[test]
fn matching_independent_reference_classifies_only_capacity_resemblance() {
    let b = fixture(1, Some("standard")).evidence();
    for (factor, result) in [(1, "PRO-5X-LIKE"), (2, "INCONCLUSIVE"), (4, "PRO-20X-LIKE")] {
        let mut e = fixture(factor, Some("standard")).evidence();
        e.controls.as_mut().unwrap().expected_tier = "pro_20x".into();
        let result_e = compare_with(e, &b);
        assert_eq!(result_e.assessment, result);
        assert!(result_e.boundary.contains("cannot establish"));
    }
}
#[test]
fn unknown_fast_is_user_attested_and_caps_confidence() {
    let f = fixture(1, None);
    let e = f.evidence();
    assert_eq!(e.summary.quality, "MEDIUM");
    assert!(e
        .intervals
        .iter()
        .filter(|r| r.eligible)
        .all(|r| r.fast_evidence == "USER_ATTESTED"));
    let b = fixture(1, Some("standard")).evidence();
    assert_eq!(compare_with(e, &b).confidence, "MEDIUM");
}
#[test]
fn explicit_fast_overrides_user_attestation() {
    let f = fixture(1, Some("fast"));
    let e = f.evidence();
    assert_eq!(e.summary.samples, 0);
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("non-standard"))));
}
#[test]
fn mixed_models_reasoning_context_and_subagents_are_excluded() {
    for sql in [
        "UPDATE events SET model='other-model' WHERE kind='token'",
        "UPDATE events SET effort='low' WHERE kind='token'",
        "UPDATE events SET is_subagent=1 WHERE kind='token'",
    ] {
        let f = fixture(1, Some("standard"));
        f.store.conn.execute_batch(sql).unwrap();
        assert_eq!(f.evidence().summary.samples, 0);
    }
    let f = fixture(1, Some("standard"));
    let mut config: serde_json::Value = serde_json::from_str(
        &f.store
            .conn
            .query_row("SELECT config FROM audit_runs", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    config["controls"]["context_band"] = json!("short");
    f.store
        .conn
        .execute("UPDATE audit_runs SET config=?", [config.to_string()])
        .unwrap();
    assert_eq!(f.evidence().summary.samples, 0);
}
#[test]
fn resets_decreases_gaps_and_switches_cannot_be_fit_as_consumption() {
    let f = fixture(1, Some("standard"));
    let mut q = quota(START + 10 * 3600, 25.0);
    q.weekly.as_mut().unwrap().resets_at = Some(START + 700000);
    account::save(&f.store, &q).unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("reset"))));
    assert!(e.summary.samples < 30);
    let f = fixture(1, Some("standard"));
    f.store
        .conn
        .execute(
            "DELETE FROM quota_snapshots WHERE timestamp>? AND timestamp<?",
            [START + 3600, START + 7200],
        )
        .unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("polling gap"))));
    let f = fixture(1, Some("standard"));
    let mut q = quota(START + 3900, 7.0);
    q.account_key = "another-account".into();
    account::save(&f.store, &q).unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("Account changed"))));
    let f = fixture(1, Some("standard"));
    account::save(&f.store, &quota(START + 7200, 6.0)).unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("decreased"))));
}
#[test]
fn settled_windows_and_unmatched_zero_token_drops_stay_distinct() {
    let f = fixture(1, Some("standard"));
    let e = auditor::report(&f.store, &f.c, &f.s, &quota(END, 65.0), END + 30)
        .unwrap()
        .evidence;
    assert_eq!(e.summary.samples, 29);
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("Settling"))));
    f.store
        .conn
        .execute(
            "DELETE FROM events WHERE timestamp>? AND timestamp<=?",
            [START + 3600, START + 7200],
        )
        .unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| r.reasons.iter().any(|s| s.contains("No local token"))));
}
#[test]
fn prices_are_frozen_for_running_observation() {
    let mut f = fixture(1, Some("standard"));
    let before = f.evidence();
    f.c.entries[0].input = 9999.0;
    let after = f.evidence();
    assert_eq!(before.pricing.fingerprint, after.pricing.fingerprint);
    assert_eq!(before.summary.api_equivalent, after.summary.api_equivalent);
}
#[test]
fn old_or_different_workload_or_same_origin_reference_cannot_classify() {
    let a = fixture(1, Some("standard")).evidence();
    let b = fixture(1, Some("standard")).evidence();
    let mut wrong = b.clone();
    wrong.origin_id = a.origin_id.clone();
    assert_eq!(compare_with(a.clone(), &wrong).assessment, "INCONCLUSIVE");
    let mut wrong = b.clone();
    wrong.generated_at -= 31 * 86400;
    assert_eq!(compare_with(a.clone(), &wrong).assessment, "INCONCLUSIVE");
    let mut wrong = b.clone();
    wrong.controls.as_mut().unwrap().effort = "max".into();
    assert_eq!(compare_with(a.clone(), &wrong).assessment, "INCONCLUSIVE");
    let mut wrong = b.clone();
    wrong.pricing.fingerprint = "different".into();
    assert_eq!(compare_with(a.clone(), &wrong).assessment, "INCONCLUSIVE");
    let mut wrong = b;
    wrong.summary.tokens.cached_input_tokens = 0;
    assert_eq!(compare_with(a, &wrong).assessment, "INCONCLUSIVE");
}
#[test]
fn sparse_and_unstable_samples_never_gain_high_confidence() {
    let e = fixture(1, Some("standard")).evidence();
    assert_eq!(
        auditor::summarize(&e.intervals[..3]).quality,
        "INSUFFICIENT"
    );
    let mut rows = e.intervals;
    for (i, r) in rows.iter_mut().enumerate() {
        if i % 2 == 0 {
            r.api_equivalent *= 4.0;
            r.usd_per_percent = r.usd_per_percent.map(|v| v * 4.0);
        }
    }
    assert_eq!(auditor::summarize(&rows).quality, "INSUFFICIENT");
}
#[test]
fn baseline_import_recalculates_and_rejects_tampering_overlap_and_own_origin() {
    let f = fixture(1, Some("standard"));
    let mut b = fixture(1, Some("standard")).evidence();
    b.ended_at = Some(NOW);
    let id = f.evidence().origin_id;
    let source = "https://github.com/example/evidence";
    let text = auditor::render(&b, "json").unwrap();
    auditor::import_baseline(&f.store, &text, source, &id, NOW).unwrap();
    assert!(f.view().baseline_loaded);
    assert!(auditor::import_baseline(&f.store, &text, source, &b.origin_id, NOW).is_err());
    assert!(auditor::import_baseline(
        &f.store,
        &text,
        "https://github.com/example?token=secret",
        &id,
        NOW
    )
    .is_err());
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["report"]["summary"]["api_equivalent"] = json!(99999);
    assert!(auditor::import_baseline(&f.store, &v.to_string(), source, &id, NOW).is_err());
    let mut wrong = b.clone();
    wrong.intervals[1].start = wrong.intervals[0].start;
    assert!(auditor::import_baseline(
        &f.store,
        &auditor::render(&wrong, "json").unwrap(),
        source,
        &id,
        NOW
    )
    .is_err());
    let mut wrong = b;
    wrong.intervals[1].api_equivalent *= 4.0;
    assert!(auditor::import_baseline(
        &f.store,
        &auditor::render(&wrong, "json").unwrap(),
        source,
        &id,
        NOW
    )
    .is_err());
}
#[test]
fn evidence_is_redacted_and_export_formats_carry_tokens_and_method() {
    let f = fixture(1, Some("standard"));
    let e = f.evidence();
    for format in ["json", "html", "csv"] {
        let text = auditor::render(&e, format).unwrap();
        for private in [
            "private-person",
            "private-real-session-id",
            "private-turn",
            "private-response",
            "/Users/",
            "project-secret",
            "account_key",
            "account_label",
        ] {
            assert!(!text.contains(private), "{format} leaked {private}");
        }
        assert!(!text.contains("<script"));
    }
    let csv = auditor::render(&e, "csv").unwrap();
    assert!(csv.contains("fresh_input,cached_input,output,reasoning_in_output"));
    let html = auditor::render(&e, "html").unwrap();
    assert!(!html.contains("Some("));
    assert!(html.contains("cannot establish a backend entitlement error"));
    assert!(html.contains("Method and limitations"));
}
#[test]
fn starting_requires_current_quota_and_explicit_declarations() {
    let f = fixture(1, Some("standard"));
    auditor::stop(&f.store, NOW).unwrap();
    let mut no = controls();
    no.fast_off_attested = false;
    assert!(auditor::start(&f.store, &f.q, &f.c, &f.s, no, NOW).is_err());
    let mut q = f.q.clone();
    q.meta.status = "CACHED".into();
    assert!(auditor::start(&f.store, &q, &f.c, &f.s, controls(), NOW).is_err());
    auditor::start(&f.store, &f.q, &f.c, &f.s, controls(), NOW).unwrap();
    assert!(auditor::start(&f.store, &f.q, &f.c, &f.s, controls(), NOW).is_err());
}
#[test]
fn migration_from_v2_preserves_events_offsets_preferences_and_snapshots() {
    let f = fixture(1, Some("standard"));
    let before = f.store.events(0, i64::MAX).unwrap();
    let path = f.store.path.clone();
    f.store.conn.execute_batch("INSERT INTO settings VALUES('test-preference','keep'); ALTER TABLE events DROP COLUMN service_tier; ALTER TABLE events DROP COLUMN is_subagent; DROP TABLE audit_runs; DROP TABLE audit_origins; PRAGMA user_version=2;").unwrap();
    let offsets: String = f
        .store
        .conn
        .query_row("SELECT group_concat(offset) FROM files", [], |r| r.get(0))
        .unwrap();
    drop(f.store);
    let store = Store::open(&path).unwrap();
    let after = store.events(0, i64::MAX).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(
        before.iter().map(|e| e.tokens.total_tokens).sum::<u64>(),
        after.iter().map(|e| e.tokens.total_tokens).sum::<u64>()
    );
    assert!(after
        .iter()
        .all(|e| e.service_tier.is_none() && e.is_subagent.is_none()));
    assert_eq!(
        store
            .conn
            .query_row("SELECT group_concat(offset) FROM files", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        offsets
    );
    assert_eq!(
        store
            .conn
            .query_row("SELECT COUNT(*) FROM quota_snapshots", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        364
    );
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='test-preference'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "keep"
    );
}
#[test]
fn parser_does_not_carry_fast_off_into_a_new_unknown_turn() {
    let mut state = ParserState::default();
    let parse = |v: serde_json::Value, s: &mut ParserState| {
        parser::parse(v.to_string().as_bytes(), s).unwrap()
    };
    parse(
        json!({"type":"turn_context","payload":{"model":"m","service_tier":"standard"}}),
        &mut state,
    );
    assert_eq!(state.service_tier.as_deref(), Some("standard"));
    parse(
        json!({"type":"turn_context","payload":{"model":"m"}}),
        &mut state,
    );
    assert_eq!(state.service_tier, None);
}

#[test]
fn reset_credit_between_unchanged_polls_is_not_lost() {
    let f = fixture(1, Some("standard"));
    for (t, n) in [(START + 300, 1), (START + 600, 0)] {
        let mut q = quota(t, 5.0);
        q.reset_credits = Some(account::ResetCredits {
            available_count: n,
            details: Some(vec![]),
        });
        account::save(&f.store, &q).unwrap();
    }
    let e = f.evidence();
    assert!(e.intervals[0]
        .reasons
        .iter()
        .any(|s| s == "Reset credit consumed"));
    assert!(!e.intervals[0].eligible);
}

#[test]
fn stale_current_quota_prevents_classification_and_private_reference_is_supported() {
    let mut f = fixture(1, Some("standard"));
    let mut b = fixture(1, Some("standard")).evidence();
    b.ended_at = Some(NOW);
    auditor::import_baseline(
        &f.store,
        &auditor::render(&b, "json").unwrap(),
        "private-reference",
        &f.evidence().origin_id,
        NOW,
    )
    .unwrap();
    assert_eq!(f.evidence().assessment, "PRO-5X-LIKE");
    f.q.meta.status = "CACHED".into();
    let e = f.evidence();
    assert_eq!(e.assessment, "INCONCLUSIVE");
    assert_eq!(e.confidence, "INSUFFICIENT");
    assert!(e.assessment_reasons.iter().any(|s| s.contains("stale")));
}

#[test]
fn saturation_and_new_parser_errors_cannot_create_a_tier_signal() {
    let f = fixture(1, Some("standard"));
    account::save(&f.store, &quota(END, 99.0)).unwrap();
    let e = f.evidence();
    assert!(e
        .intervals
        .iter()
        .any(|r| !r.eligible && r.reasons.iter().any(|s| s.contains("saturation"))));
    f.store
        .conn
        .execute_batch("UPDATE files SET errors=errors+1")
        .unwrap();
    assert_eq!(f.evidence().summary.samples, 0);
}

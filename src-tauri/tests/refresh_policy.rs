//! Synthetic scheduling and fallback checks: no provider or user logs are opened.
#[path = "../src/refresh_policy.rs"]
mod refresh_policy;
use codexmeter_core::{ingest, storage::Store};
use refresh_policy::*;
use std::{fs, io::Write, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "codexmeter-refresh-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn idle_polling_is_bounded_and_real_activity_restores_configured_cadence() {
    let mut p = RefreshPolicy::new(1000);
    assert_eq!(p.interval(1299, 90, true, false), 90);
    assert_eq!(p.interval(1300, 90, true, false), 180);
    assert_eq!(p.interval(1900, 90, true, false), 300);
    p.observe_ingest(2000, 0, 0);
    assert_eq!(p.interval(2000, 90, true, false), 300);
    p.observe_ingest(2001, 18, 0);
    assert_eq!(p.interval(2001, 90, true, false), 90);
    p.observe_ingest(4000, 0, 1);
    assert_eq!(p.interval(4000, 90, true, false), 90);
    assert_eq!(p.interval(9000, 3600, true, false), 3600);
    assert_eq!(p.interval(0, 90, true, false), 90);
}

#[test]
fn audit_and_disabled_adaptive_always_keep_configured_cadence() {
    let p = RefreshPolicy::new(0);
    for base in [60, 90, 300, 3600] {
        assert_eq!(p.interval(100_000, base, true, true), base);
        assert_eq!(p.interval(100_000, base, false, false), base);
    }
}

#[test]
fn manual_refresh_and_changed_intervals_cannot_bypass_failure_backoff() {
    assert_eq!(next_attempt(100, 90, 0, false), 190);
    assert_eq!(next_attempt(100, 90, 0, true), 115);
    assert_eq!(next_attempt(100, 300, 1000, false), 1000);
    assert_eq!(next_attempt(100, 60, 1000, true), 1000);
    assert_eq!(freshness_seconds(60, true), 360);
    assert_eq!(freshness_seconds(60, false), 180);
    assert_eq!(freshness_seconds(3600, true), 10_800);
}

#[test]
fn watch_failure_has_fast_reconciliation_and_bounded_reattachment() {
    let mut r = RecoveryPolicy::default();
    assert!(r.may_attach(0));
    r.watch_attached();
    assert!(!r.may_attach(999));
    assert_eq!(r.reconcile_seconds(), NATIVE_RECONCILE_SECONDS);
    r.watch_failed(1000);
    assert_eq!(r.reconcile_seconds(), FALLBACK_RECONCILE_SECONDS);
    assert!(r.status().starts_with("watcher degraded"));
    assert!(!r.may_attach(1299));
    assert!(r.may_attach(1300));
    r.watch_attached();
    assert_eq!(r.status(), "watching");
    r.roots_changed();
    assert!(r.may_attach(1301));
    assert!(!r.native_ready);
    assert!(WATCH_CHECK_SECONDS <= FALLBACK_RECONCILE_SECONDS);
}

#[test]
fn ingestion_failure_retries_within_one_minute_and_remains_distinct_from_watcher_health() {
    let mut r = RecoveryPolicy::default();
    r.watch_attached();
    r.ingest_failed(100);
    assert_eq!(r.retry_ingest_at, 105);
    assert_eq!(r.status(), "ingest error; retrying automatically");
    assert!(r.native_ready);
    assert!(!r.ingest_is_healthy());
    for _ in 0..100 {
        r.ingest_failed(200);
        assert!((205..=260).contains(&r.retry_ingest_at));
    }
    r.watch_failed(200);
    assert_eq!(r.status(), "ingest error; retrying automatically");
    r.ingest_succeeded();
    assert_eq!(r.retry_ingest_at, 0);
    assert!(r.ingest_is_healthy());
    assert!(r.status().starts_with("watcher degraded"));
    r.ingest_failed(300);
    assert_eq!(r.retry_ingest_at, 305);
}

#[test]
fn native_budget_counts_directories_non_logs_and_new_roots() {
    let temp = Temp::new();
    assert!(existing_roots(&temp.0).is_empty());
    let root = temp.0.join("sessions");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("nested/non-log.txt"), "synthetic").unwrap();
    assert_eq!(existing_roots(&temp.0), vec![root.clone()]);
    assert!(tree_within_budget(&existing_roots(&temp.0), 3));
    assert!(!tree_within_budget(&existing_roots(&temp.0), 2));
    fs::create_dir_all(temp.0.join("archived_sessions")).unwrap();
    assert_eq!(existing_roots(&temp.0).len(), 2);
    assert!(!tree_within_budget(&existing_roots(&temp.0), 3));
    assert!(tree_within_budget(&existing_roots(&temp.0), 4));
}

#[test]
#[cfg(unix)]
fn native_budget_rejects_links_instead_of_following_them_outside_log_trees() {
    let temp = Temp::new();
    let outside = Temp::new();
    fs::create_dir_all(temp.0.join("sessions")).unwrap();
    std::os::unix::fs::symlink(&outside.0, temp.0.join("sessions/link")).unwrap();
    assert!(!tree_within_budget(&existing_roots(&temp.0), 128));
}

#[test]
fn fallback_discovers_roots_created_later_and_reconciles_only_appended_bytes() {
    let temp = Temp::new();
    let mut store = Store::open(&temp.0.join("db")).unwrap();
    assert!(ingest::discover(&temp.0).is_empty());
    let root = temp.0.join("sessions");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("rollout-synthetic.jsonl");
    let meta = "{\"timestamp\":\"2026-09-10T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"synthetic-fallback\"}}\n";
    let record = |raw: u64, total: u64| {
        serde_json::json!({"timestamp":"2026-09-10T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":raw,"cached_input_tokens":0,"output_tokens":10},"total_token_usage":{"input_tokens":total,"output_tokens":10}}}}).to_string()+"\n"
    };
    fs::write(&file, meta.to_owned() + &record(100, 100)).unwrap();
    let first = ingest::ingest(&mut store, &ingest::discover(&temp.0)).unwrap();
    assert!(first.bytes_read > 0);
    let same = ingest::ingest(&mut store, &ingest::discover(&temp.0)).unwrap();
    assert_eq!(same.bytes_read, 0);
    fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(record(50, 150).as_bytes())
        .unwrap();
    let append = ingest::ingest(&mut store, &ingest::discover(&temp.0)).unwrap();
    assert!(append.bytes_read > 0 && append.bytes_read < fs::metadata(&file).unwrap().len());
    assert_eq!(
        store
            .events(0, i64::MAX)
            .unwrap()
            .iter()
            .map(|e| e.tokens.total_tokens)
            .sum::<u64>(),
        170
    );
    let mut pending = ingest::discover(&temp.0).into_iter().collect();
    assert!(!ingest_pending(&mut store, &mut pending).index_changed);
    fs::write(&file, "").unwrap();
    let mut pending = ingest::discover(&temp.0).into_iter().collect();
    let truncated = ingest_pending(&mut store, &mut pending);
    assert_eq!(truncated.bytes_read, 0);
    assert!(
        truncated.index_changed,
        "zero-byte truncation must refresh displayed totals"
    );
    assert!(store.events(0, i64::MAX).unwrap().is_empty());
    drop(store);
}

#[test]
#[cfg(unix)]
fn unreadable_file_is_retried_without_blocking_other_sessions_or_duplicating_usage() {
    use std::{collections::BTreeSet, os::unix::fs::PermissionsExt};
    let temp = Temp::new();
    let root = temp.0.join("sessions");
    fs::create_dir_all(&root).unwrap();
    let blocked = root.join("a-blocked.jsonl");
    let healthy = root.join("z-healthy.jsonl");
    let record = |id: &str, input: u64| {
        let meta = serde_json::json!({"timestamp":"2026-09-10T10:00:00Z","type":"session_meta","payload":{"id":id}});
        let usage = serde_json::json!({"timestamp":"2026-09-10T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":input,"cached_input_tokens":0,"output_tokens":10}}}});
        format!("{meta}\n{usage}\n")
    };
    fs::write(&blocked, record("synthetic-blocked", 100)).unwrap();
    fs::write(&healthy, record("synthetic-healthy", 50)).unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0)).unwrap();
    if fs::File::open(&blocked).is_ok() {
        // Root can bypass this fixture's permissions; other fallback tests still run.
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o600)).unwrap();
        return;
    }
    let mut store = Store::open(&temp.0.join("db")).unwrap();
    let mut pending: BTreeSet<_> = ingest::discover(&temp.0).into_iter().collect();
    let partial = ingest_pending(&mut store, &mut pending);
    assert!(!partial.complete);
    assert!(partial.bytes_read > 0);
    assert_eq!(pending, [blocked.clone()].into_iter().collect());
    assert!(
        !retry_pending(&mut store, &mut pending).index_changed,
        "an unchanged persistent read error should not trigger another data notification"
    );
    assert_eq!(
        store
            .events(0, i64::MAX)
            .unwrap()
            .iter()
            .map(|e| e.tokens.total_tokens)
            .sum::<u64>(),
        60
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o600)).unwrap();
    let recovered = ingest_pending(&mut store, &mut pending);
    assert!(recovered.complete && pending.is_empty());
    let mut verify = ingest::discover(&temp.0).into_iter().collect();
    assert_eq!(ingest_pending(&mut store, &mut verify).bytes_read, 0);
    assert_eq!(
        store
            .events(0, i64::MAX)
            .unwrap()
            .iter()
            .map(|e| e.tokens.total_tokens)
            .sum::<u64>(),
        170
    );
    drop(store);
}

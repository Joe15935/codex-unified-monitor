use codexmeter_core::{
    accounting::Tokens,
    ingest::{bounded_line, ingest},
    storage::Store,
};
use serde_json::json;
use std::{fs, io::Write};
fn line(v: serde_json::Value) -> String {
    v.to_string() + "\n"
}
fn meta() -> String {
    line(
        json!({"timestamp":"2026-09-10T10:00:00Z","type":"session_meta","payload":{"id":"sample-session","cwd":"/synthetic/project"}}),
    ) + &line(
        json!({"timestamp":"2026-09-10T10:00:00Z","type":"turn_context","payload":{"model":"gpt-5.6-sol","effort":"high"}}),
    )
}
fn usage(raw: u64, cached: u64, out: u64) -> serde_json::Value {
    json!({"input_tokens":raw,"cached_input_tokens":cached,"output_tokens":out,"reasoning_output_tokens":2,"total_tokens":raw+out})
}
fn token(raw: u64, cached: u64, out: u64, total: u64) -> String {
    line(
        json!({"timestamp":"2026-09-10T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":usage(raw,cached,out),"total_token_usage":usage(total,0,out)}}}),
    )
}
fn total(s: &Store) -> u64 {
    s.events(0, i64::MAX)
        .unwrap()
        .iter()
        .map(|e| e.tokens.total_tokens)
        .sum()
}
#[test]
fn canonical_no_reasoning_double_count() {
    let t = Tokens::new(100, 60, 20, 10, 0).unwrap();
    assert_eq!(t.total_tokens, 120);
    assert_eq!(t.uncached_input_tokens, 40);
    assert_eq!(t.cache_ratio(), Some(0.6));
}
#[test]
fn invalid_counters_rejected() {
    assert!(Tokens::new(10, 11, 0, 0, 0).is_err());
    assert!(Tokens::new(10, 0, 1, 2, 0).is_err());
    assert!(Tokens::from_usage(&json!({"input_tokens":-1})).is_err());
}
#[test]
fn zero_input_has_no_ratio() {
    assert_eq!(Tokens::default().cache_ratio(), None);
}
#[test]
fn append_restart_rename_archive_and_rescan_are_idempotent() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("data.db");
    let f = d.path().join("rollout-one.jsonl");
    fs::write(&f, meta() + &token(100, 60, 20, 100)).unwrap();
    let mut s = Store::open(&db).unwrap();
    ingest(&mut s, &[f.clone()]).unwrap();
    assert_eq!(total(&s), 120);
    assert_eq!(ingest(&mut s, &[f.clone()]).unwrap().bytes_read, 0);
    fs::OpenOptions::new()
        .append(true)
        .open(&f)
        .unwrap()
        .write_all(token(50, 30, 10, 150).as_bytes())
        .unwrap();
    let r = ingest(&mut s, &[f.clone()]).unwrap();
    assert!(r.bytes_read < fs::metadata(&f).unwrap().len());
    assert_eq!(total(&s), 180);
    drop(s);
    let mut s = Store::open(&db).unwrap();
    assert_eq!(ingest(&mut s, &[f.clone()]).unwrap().bytes_read, 0);
    let archive = d.path().join("archive.jsonl");
    fs::rename(&f, &archive).unwrap();
    assert_eq!(ingest(&mut s, &[archive.clone()]).unwrap().bytes_read, 0);
    fs::copy(&archive, &f).unwrap();
    assert!(ingest(&mut s, &[f]).unwrap().duplicates > 0);
    assert_eq!(total(&s), 180);
}
#[test]
fn repeated_cumulative_not_added() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("data.db")).unwrap();
    let f = d.path().join("x.jsonl");
    let t = token(100, 40, 20, 100);
    fs::write(&f, meta() + &t + &t).unwrap();
    ingest(&mut s, &[f]).unwrap();
    assert_eq!(total(&s), 120);
}
#[test]
fn modern_and_legacy_records_reconcile() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("db")).unwrap();
    let f = d.path().join("x.jsonl");
    let modern = line(
        json!({"timestamp":"2026-09-10T10:01:00Z","type":"token_usage_record","payload":{"thread_id":"sample-session","response_id":"resp-synthetic","usage":usage(100,40,20),"thread_token_usage":usage(100,0,20)}}),
    );
    fs::write(&f, meta() + &modern + &token(100, 40, 20, 100)).unwrap();
    ingest(&mut s, &[f]).unwrap();
    assert_eq!(total(&s), 120);
    assert_eq!(
        s.events(0, i64::MAX).unwrap()[0].source,
        "token_usage_record"
    );
}
#[test]
fn partial_line_waits_for_newline() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("db")).unwrap();
    let f = d.path().join("x.jsonl");
    let t = token(100, 40, 20, 100);
    fs::write(&f, meta() + t.trim_end()).unwrap();
    ingest(&mut s, &[f.clone()]).unwrap();
    assert_eq!(total(&s), 0);
    fs::OpenOptions::new()
        .append(true)
        .open(&f)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    ingest(&mut s, &[f]).unwrap();
    assert_eq!(total(&s), 120);
}
#[test]
fn truncate_and_same_size_rewrite_replace_source() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("db")).unwrap();
    let f = d.path().join("x.jsonl");
    fs::write(&f, meta() + &token(100, 40, 20, 100)).unwrap();
    let original = fs::metadata(&f).unwrap();
    ingest(&mut s, &[f.clone()]).unwrap();
    fs::write(&f, meta() + &token(200, 40, 20, 200)).unwrap();
    // Rapid writes can share an mtime on Windows. Make the changed-metadata
    // precondition deterministic without depending on filesystem clock ticks.
    fs::OpenOptions::new()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(original.modified().unwrap() + std::time::Duration::from_secs(60))
        .unwrap();
    let rewritten = fs::metadata(&f).unwrap();
    assert_eq!(rewritten.len(), original.len());
    assert_ne!(rewritten.modified().unwrap(), original.modified().unwrap());
    ingest(&mut s, &[f.clone()]).unwrap();
    assert_eq!(total(&s), 220);
    fs::write(&f, meta()).unwrap();
    ingest(&mut s, &[f]).unwrap();
    assert_eq!(total(&s), 0);
}
#[test]
fn large_line_allocation_is_bounded() {
    let bytes = vec![b'x'; 100000];
    let mut data = bytes;
    data.push(b'\n');
    let mut r = std::io::Cursor::new(data);
    let (v, n, complete, large) = bounded_line(&mut r, 128).unwrap();
    assert!(v.len() <= 128);
    assert_eq!(n, 100001);
    assert!(complete && large);
}
#[test]
fn metadata_only_database() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("db")).unwrap();
    let f = d.path().join("x.jsonl");
    let private = line(
        json!({"type":"response_item","payload":{"type":"message","content":"SYNTHETIC_PRIVATE_SENTINEL"}}),
    );
    fs::write(&f, meta() + &private + &token(100, 40, 20, 100)).unwrap();
    ingest(&mut s, &[f]).unwrap();
    let text = serde_json::to_string(&s.events(0, i64::MAX).unwrap()).unwrap();
    assert!(!text.contains("SYNTHETIC_PRIVATE_SENTINEL"));
}

#[test]
fn child_id_wins_over_root_lineage_and_fork_snapshot_is_not_new_usage() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(&d.path().join("db")).unwrap();
    let root = d.path().join("root.jsonl");
    let child = d.path().join("child.jsonl");
    let fork = d.path().join("fork.jsonl");
    fs::write(&root, meta() + &token(100, 40, 20, 100)).unwrap();
    let child_meta = line(
        json!({"timestamp":"2026-09-10T10:00:00Z","type":"session_meta","payload":{"id":"child","session_id":"sample-session","parent_thread_id":"sample-session"}}),
    );
    fs::write(&child, child_meta + &token(100, 40, 20, 100)).unwrap();
    let fork_meta = line(
        json!({"timestamp":"2026-09-10T10:01:00Z","type":"session_meta","payload":{"id":"fork","session_id":"sample-session","forked_from_id":"sample-session"}}),
    );
    // Fork snapshots embed the parent's header and rewrite history timestamps.
    let snapshot = meta() + &token(100, 40, 20, 100);
    let new_usage = token(50, 30, 10, 150).replace("10:01:00Z", "10:01:00.001Z");
    fs::write(&fork, fork_meta + &snapshot + &new_usage).unwrap();
    ingest(&mut s, &[fork, root, child]).unwrap();
    let events = s.events(0, i64::MAX).unwrap();
    assert_eq!(total(&s), 300);
    for (sid, expected) in [("sample-session", 120), ("child", 120), ("fork", 60)] {
        assert_eq!(
            events
                .iter()
                .filter(|e| e.session_id == sid)
                .map(|e| e.tokens.total_tokens)
                .sum::<u64>(),
            expected
        );
    }
}

#[test]
fn schema_migration_preserves_settings_and_quota_history() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("db");
    let s = Store::open(&path).unwrap();
    s.conn.execute_batch("INSERT INTO settings VALUES('timezone','UTC'); INSERT INTO quota_snapshots VALUES(1,'synthetic','{}'); ALTER TABLE events DROP COLUMN service_tier; ALTER TABLE events DROP COLUMN is_subagent; DROP TABLE audit_runs; DROP TABLE audit_origins; PRAGMA user_version=1;").unwrap();
    drop(s);
    let s = Store::open(&path).unwrap();
    let value: String = s
        .conn
        .query_row("SELECT value FROM settings WHERE key='timezone'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "UTC");
    assert_eq!(
        s.conn
            .query_row("SELECT COUNT(*) FROM quota_snapshots", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

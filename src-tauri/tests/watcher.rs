//! Actual OS notifications, with synthetic files outside the user's Codex home.
use codexmeter_core::{ingest, storage::Store};
use notify::Watcher;
use std::{
    fs,
    io::Write,
    time::{Duration, Instant},
};
#[test]
fn native_watcher_delivers_creation_and_later_append_to_incremental_index() {
    let temp = std::env::temp_dir().join(format!(
        "codexmeter-watch-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp.join("sessions")).unwrap();
    let root = temp.join("sessions");
    let file = root.join("rollout-synthetic.jsonl");
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            let _ = tx.send(event);
        })
        .unwrap();
    watcher
        .watch(&root, notify::RecursiveMode::Recursive)
        .unwrap();
    let wait = || {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let event = rx
                .recv_timeout(end.saturating_duration_since(Instant::now()))
                .expect("OS notification missing")
                .unwrap();
            if event
                .paths
                .iter()
                .any(|p| p.file_name() == file.file_name())
            {
                break;
            }
        }
    };
    let meta="{\"timestamp\":\"2026-09-10T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"synthetic-watch\"}}\n";
    let record = |raw: u64, total: u64| {
        serde_json::json!({"timestamp":"2026-09-10T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":raw,"cached_input_tokens":0,"output_tokens":10},"total_token_usage":{"input_tokens":total,"output_tokens":10}}}}).to_string()+"\n"
    };
    fs::write(&file, meta.to_owned() + &record(100, 100)).unwrap();
    wait();
    let mut store = Store::open(&temp.join("db")).unwrap();
    ingest::ingest(&mut store, &[file.clone()]).unwrap();
    while rx.try_recv().is_ok() {}
    fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(record(50, 150).as_bytes())
        .unwrap();
    wait();
    let result = ingest::ingest(&mut store, &[file.clone()]).unwrap();
    assert!(result.bytes_read < fs::metadata(&file).unwrap().len());
    assert_eq!(
        store
            .events(0, i64::MAX)
            .unwrap()
            .iter()
            .map(|e| e.tokens.total_tokens)
            .sum::<u64>(),
        170
    );
    drop(watcher);
    drop(store);
    fs::remove_dir_all(temp).unwrap();
}

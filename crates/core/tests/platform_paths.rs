use codexmeter_core::{ingest::ingest, storage::Store};
use std::fs::{self, File, FileTimes};

#[test]
fn same_size_same_timestamp_replacement_uses_new_file_identity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout 中文 with spaces.jsonl");
    let content = |input: u64| {
        let header = serde_json::json!({"timestamp":"2026-09-10T10:00:00Z","type":"session_meta","payload":{"id":"synthetic-replacement"}});
        let event = serde_json::json!({"timestamp":"2026-09-10T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":input,"output_tokens":10},"total_token_usage":{"input_tokens":input,"output_tokens":10}}}});
        format!("{header}\r\n{event}\r\n")
    };
    fs::write(&path, content(100)).unwrap();
    let metadata = fs::metadata(&path).unwrap();
    let mut store = Store::open(&temp.path().join("index.sqlite3")).unwrap();
    ingest(&mut store, &[path.clone()]).unwrap();
    assert_eq!(
        store.events(0, i64::MAX).unwrap()[0]
            .tokens
            .raw_input_tokens,
        100
    );
    fs::rename(&path, temp.path().join("old-file")).unwrap();
    fs::write(&path, content(900)).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    assert_eq!(metadata.len(), fs::metadata(&path).unwrap().len());
    assert_eq!(
        metadata.modified().unwrap(),
        fs::metadata(&path).unwrap().modified().unwrap()
    );
    ingest(&mut store, &[path]).unwrap();
    let events = store.events(0, i64::MAX).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tokens.raw_input_tokens, 900);
}

#[test]
fn application_data_uses_platform_private_data_location() {
    #[cfg(windows)]
    let base = dirs::data_local_dir().unwrap();
    #[cfg(not(windows))]
    let base = dirs::data_dir().unwrap();
    assert_eq!(
        codexmeter_core::data_dir(),
        base.join("Codex Unified Monitor")
    );
}

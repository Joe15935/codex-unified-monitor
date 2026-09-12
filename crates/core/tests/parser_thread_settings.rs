use codexmeter_core::{
    ingest,
    parser::{self, Event, ParserState},
    storage::Store,
};
use serde_json::{json, Value};
use std::fs;

fn parse(value: Value, state: &mut ParserState) -> Option<Event> {
    parser::parse(&serde_json::to_vec(&value).unwrap(), state).unwrap()
}

fn header() -> Value {
    json!({"timestamp":"2026-09-12T10:00:00Z","type":"session_meta","payload":{
        "id":"test-thread","source":"cli"
    }})
}

fn context(tier: Option<Value>) -> Value {
    let mut value = json!({"timestamp":"2026-09-12T10:01:00Z","type":"turn_context","payload":{
        "model":"unknown-test-model","effort":"high","turn_id":"turn-1"
    }});
    if let Some(tier) = tier {
        value["payload"]["service_tier"] = tier;
    }
    value
}

fn settings(tier: Option<Value>) -> Value {
    let mut value = json!({"timestamp":"2026-09-12T10:01:01Z","type":"event_msg","payload":{
        "type":"thread_settings_applied","thread_id":"test-thread","thread_settings":{}
    }});
    if let Some(tier) = tier {
        value["payload"]["thread_settings"]["service_tier"] = tier;
    }
    value
}

fn token() -> Value {
    json!({"timestamp":"2026-09-12T10:02:00Z","type":"token_usage_record","payload":{
        "thread_id":"test-thread","turn_id":"turn-1","response_id":"response-1",
        "usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":20,"reasoning_output_tokens":5}
    }})
}

fn state() -> ParserState {
    let mut state = ParserState::default();
    parse(header(), &mut state);
    parse(context(None), &mut state);
    state
}

#[test]
fn applied_tiers_preserve_wire_values_without_changing_accounting_or_model() {
    for tier in ["default", "standard", "fast", "priority", "flex", "batch"] {
        let mut state = state();
        assert!(parse(settings(Some(json!(tier))), &mut state).is_none());
        let event = parse(token(), &mut state).unwrap();
        assert_eq!(event.service_tier.as_deref(), Some(tier));
        assert_eq!(event.model, "unknown-test-model");
        assert_eq!(event.tokens.raw_input_tokens, 100);
        assert_eq!(event.tokens.cached_input_tokens, 80);
        assert_eq!(event.tokens.output_tokens, 20);
        assert_eq!(event.tokens.reasoning_output_tokens, 5);
        assert_eq!(event.tokens.total_tokens, 120);
    }
}

#[test]
fn missing_setting_field_adds_no_evidence_but_explicit_unknown_clears_it() {
    let mut state = state();
    parse(settings(None), &mut state);
    assert_eq!(parse(token(), &mut state).unwrap().service_tier, None);
    for invalid in [
        Value::Null,
        json!("auto"),
        json!("future-tier"),
        json!(""),
        json!(42),
        json!(false),
    ] {
        parse(settings(Some(json!("standard"))), &mut state);
        parse(settings(None), &mut state);
        assert_eq!(state.service_tier.as_deref(), Some("standard"));
        parse(settings(Some(invalid)), &mut state);
        assert_eq!(parse(token(), &mut state).unwrap().service_tier, None);
    }
}

#[test]
fn each_turn_context_replaces_settings_evidence_including_missing_metadata() {
    let mut state = state();
    parse(settings(Some(json!("standard"))), &mut state);
    parse(context(None), &mut state);
    assert_eq!(parse(token(), &mut state).unwrap().service_tier, None);
    parse(settings(Some(json!("standard"))), &mut state);
    parse(context(Some(json!("priority"))), &mut state);
    assert_eq!(
        parse(token(), &mut state).unwrap().service_tier.as_deref(),
        Some("priority")
    );
    parse(context(Some(json!("future-tier"))), &mut state);
    assert_eq!(parse(token(), &mut state).unwrap().service_tier, None);
}

#[test]
fn explicit_request_tier_overrides_current_settings_even_when_unknown() {
    let mut state = state();
    parse(settings(Some(json!("standard"))), &mut state);
    for tier in [Value::Null, json!("auto"), json!("future-tier"), json!(19)] {
        let mut value = token();
        value["payload"]["service_tier"] = tier;
        assert_eq!(parse(value, &mut state).unwrap().service_tier, None);
    }
    let mut value = token();
    value["payload"]["service_tier"] = json!("priority");
    assert_eq!(
        parse(value, &mut state).unwrap().service_tier.as_deref(),
        Some("priority")
    );
    // A request-specific override does not change the settings for other events.
    assert_eq!(state.service_tier.as_deref(), Some("standard"));
}

#[test]
fn copied_foreign_settings_and_known_fork_history_do_not_supply_evidence() {
    let mut state = state();
    let mut value = settings(Some(json!("standard")));
    value["payload"]["thread_id"] = json!("parent-thread");
    parse(value, &mut state);
    assert_eq!(state.service_tier, None);
    state.fork_boundary_ms = Some(
        chrono::DateTime::parse_from_rfc3339("2026-09-12T10:01:01Z")
            .unwrap()
            .timestamp_millis(),
    );
    parse(settings(Some(json!("standard"))), &mut state);
    assert_eq!(state.service_tier, None);
    let mut value = settings(Some(json!("priority")));
    value["timestamp"] = json!("2026-09-12T10:01:02Z");
    parse(value, &mut state);
    assert_eq!(state.service_tier.as_deref(), Some("priority"));
}

#[test]
fn old_ownerless_settings_require_session_metadata_and_valid_timestamp() {
    let mut value = settings(Some(json!("standard")));
    value["payload"]
        .as_object_mut()
        .unwrap()
        .remove("thread_id");
    let mut unbound = ParserState::default();
    parse(value.clone(), &mut unbound);
    assert_eq!(unbound.service_tier, None);
    let mut state = state();
    parse(value.clone(), &mut state);
    assert_eq!(state.service_tier.as_deref(), Some("standard"));
    value["timestamp"] = json!("invalid");
    value["payload"]["thread_settings"]["service_tier"] = json!("priority");
    assert!(parser::parse(&serde_json::to_vec(&value).unwrap(), &mut state).is_err());
    assert_eq!(state.service_tier.as_deref(), Some("standard"));
}

#[test]
fn incremental_resume_preserves_current_evidence_without_recounting_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("rollout.jsonl");
    let db = dir.path().join("monitor.sqlite3");
    let prefix = [header(), context(None), settings(Some(json!("standard")))]
        .iter()
        .map(|v| format!("{v}\n"))
        .collect::<String>();
    fs::write(&log, &prefix).unwrap();
    {
        let mut store = Store::open(&db).unwrap();
        ingest::ingest(&mut store, std::slice::from_ref(&log)).unwrap();
    }
    fs::write(&log, format!("{prefix}{}\n", token())).unwrap();
    let mut store = Store::open(&db).unwrap();
    ingest::ingest(&mut store, std::slice::from_ref(&log)).unwrap();
    ingest::ingest(&mut store, std::slice::from_ref(&log)).unwrap();
    let row: (i64, i64, Option<String>) = store.conn.query_row(
        "SELECT COUNT(*),SUM(raw_input+output),MIN(service_tier) FROM events WHERE kind='token'",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(row, (1, 120, Some("standard".into())));
}

//! Port of CodexScope's session/turn-state approach, extended for current response records.
//! No prompt, reasoning body, tool arguments or tool output is retained.
use crate::{accounting::Tokens, digest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ParserState {
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub effort: String,
    pub turn_id: String,
    pub started_at: i64,
    pub previous_total: Option<u64>,
    pub epoch: u64,
    pub metadata_seen: bool,
    pub fork_boundary_ms: Option<i64>,
    pub inherited_skipped: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub response_id: Option<String>,
    pub session_id: String,
    pub timestamp: i64,
    pub model: String,
    pub effort: String,
    pub turn_id: String,
    pub kind: String,
    pub tool: String,
    pub tokens: Tokens,
    pub source: String,
}
fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn normalized_model(v: &str) -> String {
    match v.trim() {
        "" | "codex" => "unknown".into(),
        x => x.to_owned(),
    }
}
pub fn parse(line: &[u8], state: &mut ParserState) -> anyhow::Result<Option<Event>> {
    let v: Value = serde_json::from_slice(line)?;
    let typ = s(&v, "type");
    let p = &v["payload"];
    let parsed_time = v["timestamp"]
        .as_str()
        .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok());
    let ts = parsed_time.map(|t| t.timestamp());
    if typ == "session_meta" {
        // A fork snapshot may embed a second, historical parent session_meta.
        // The first header identifies this file's thread. session_id is lineage
        // in current Codex logs; id is the actual thread identity.
        if state.metadata_seen {
            return Ok(None);
        }
        state.metadata_seen = true;
        let id = p["id"]
            .as_str()
            .or(p["session_id"].as_str())
            .unwrap_or(&state.session_id)
            .to_owned();
        if id != state.session_id {
            state.previous_total = None;
            state.epoch = 0;
            state.turn_id.clear();
        }
        state.session_id = id;
        if let Some(cwd) = p["cwd"].as_str() {
            state.project = cwd.to_owned();
        }
        state.started_at = p["timestamp"]
            .as_str()
            .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok())
            .map(|t| t.timestamp())
            .or(ts)
            .unwrap_or(0);
        if p["forked_from_id"].as_str().is_some_and(|s| !s.is_empty()) {
            state.fork_boundary_ms = parsed_time.map(|t| t.timestamp_millis());
        }
        return Ok(None);
    }
    if typ == "turn_context" {
        if let Some(m) = p["model"].as_str() {
            state.model = normalized_model(m);
        }
        state.effort = p["effort"]
            .as_str()
            .or(p["reasoning_effort"].as_str())
            .or(p["collaboration_mode"]["settings"]["reasoning_effort"].as_str())
            .unwrap_or(&state.effort)
            .to_owned();
        if let Some(id) = p["turn_id"].as_str() {
            state.turn_id = id.to_owned();
        }
        return Ok(None);
    }
    let token_record = typ == "token_usage_record";
    let token_msg = typ == "event_msg" && p["type"] == "token_count" && p["info"].is_object();
    let tool_type = s(p, "type");
    let is_tool =
        typ == "response_item" && (tool_type.ends_with("_call") || tool_type == "custom_tool_call");
    let is_turn =
        typ == "event_msg" && matches!(tool_type.as_str(), "task_started" | "turn_started");
    if !(token_record || token_msg || is_tool || is_turn) {
        return Ok(None);
    }
    let timestamp = ts.ok_or_else(|| anyhow::anyhow!("invalid_event_timestamp"))?;
    // Codex writes inherited rollout items at the fork-header timestamp (or
    // preserves their earlier timestamps). They seed context/cumulative state,
    // but are not new inference in this thread. Keep millisecond precision.
    let inherited = state.fork_boundary_ms.is_some_and(|boundary| {
        parsed_time.is_some_and(|time| time.timestamp_millis() <= boundary)
    });
    let mut session = state.session_id.clone();
    let mut turn = state.turn_id.clone();
    let mut response_id = None;
    let mut tokens = Tokens::default();
    let kind;
    let mut tool = String::new();
    let identity;
    if token_record || token_msg {
        kind = "token";
        let usage = if token_record {
            &p["usage"]
        } else {
            &p["info"]["last_token_usage"]
        };
        if !usage.is_object() {
            return Ok(None);
        }
        tokens = Tokens::from_usage(usage)?;
        let cumulative = if token_record {
            &p["thread_token_usage"]
        } else {
            &p["info"]["total_token_usage"]
        };
        if token_record {
            session = p["thread_id"]
                .as_str()
                .or(p["session_id"].as_str())
                .unwrap_or(&session)
                .to_owned();
            turn = p["turn_id"].as_str().unwrap_or(&turn).to_owned();
            response_id = p["response_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
        }
        if cumulative.is_object() {
            let raw = cumulative["input_tokens"].as_u64().unwrap_or(0);
            let output = cumulative["output_tokens"].as_u64().unwrap_or(0);
            let total = raw.saturating_add(output);
            if session == state.session_id {
                if state.previous_total.is_some_and(|x| total < x) {
                    state.epoch += 1;
                }
                state.previous_total = Some(total);
            }
            identity = json!([
                "token",
                session,
                state.epoch,
                raw,
                cumulative["cached_input_tokens"],
                output,
                cumulative["reasoning_output_tokens"]
            ])
            .to_string();
        } else if let Some(ref id) = response_id {
            identity = format!("response:{id}");
        } else {
            identity = json!(["token", session, timestamp, usage]).to_string();
        }
        // Compaction/resume baselines can carry cumulative usage with zero per-response counters.
        if inherited {
            state.inherited_skipped += 1;
            return Ok(None);
        }
        if tokens.total_tokens == 0 {
            return Ok(None);
        }
    } else if is_tool {
        if inherited {
            return Ok(None);
        }
        kind = "tool";
        tool = if tool_type == "function_call" || tool_type == "custom_tool_call" {
            s(p, "name")
        } else {
            tool_type.clone()
        };
        let id = p["call_id"]
            .as_str()
            .or(p["id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{timestamp}:{}:{tool}", v["ordinal"]));
        identity = format!("tool:{session}:{id}");
    } else {
        if inherited {
            return Ok(None);
        }
        kind = "turn";
        turn = p["turn_id"].as_str().unwrap_or(&turn).to_owned();
        state.turn_id = turn.clone();
        identity = format!(
            "turn:{session}:{}",
            if turn.is_empty() {
                timestamp.to_string()
            } else {
                turn.clone()
            }
        );
    }
    anyhow::ensure!(!session.is_empty(), "missing_session_identity");
    Ok(Some(Event {
        id: digest(identity),
        response_id,
        session_id: session,
        timestamp,
        model: normalized_model(&state.model),
        effort: state.effort.clone(),
        turn_id: turn,
        kind: kind.into(),
        tool,
        tokens,
        source: if token_record {
            "token_usage_record"
        } else {
            "rollout_jsonl"
        }
        .into(),
    }))
}

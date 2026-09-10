//! Read-only Rust port of zhang-mengjia's persistent app-server transport.
use crate::{digest, ingest::bounded_line, now, storage::Store};
use anyhow::{anyhow, Result};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub const READ_METHODS: [&str; 4] = [
    "initialize",
    "account/read",
    "account/rateLimits/read",
    "account/usage/read",
];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub source: String,
    pub updated_at: Option<i64>,
    pub status: String,
    pub confidence: String,
}
impl Provenance {
    pub fn new(source: &str, status: &str, at: Option<i64>) -> Self {
        Self {
            source: source.into(),
            updated_at: at,
            status: status.into(),
            confidence: if status == "LIVE" || status == "LOCAL" {
                "observed"
            } else {
                "limited"
            }
            .into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_minutes: Option<u64>,
    pub resets_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub id: String,
    pub name: Option<String>,
    pub primary: Option<Window>,
    pub secondary: Option<Window>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetCredit {
    pub status: Option<String>,
    pub reset_type: Option<String>,
    pub granted_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub title: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetCredits {
    pub available_count: u64,
    pub details: Option<Vec<ResetCredit>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub meta: Provenance,
    pub account_key: String,
    pub account_label: Option<String>,
    pub plan: Option<String>,
    pub five_hour: Option<Window>,
    pub weekly: Option<Window>,
    pub buckets: Vec<Bucket>,
    pub reset_credits: Option<ResetCredits>,
    pub credits: Option<Value>,
    pub error: Option<String>,
    pub latency_ms: Option<u128>,
    pub usage: Option<Value>,
    pub usage_status: String,
    pub next_attempt_at: Option<i64>,
}
impl Default for Quota {
    fn default() -> Self {
        Self {
            meta: Provenance::new("codex app-server", "UNAVAILABLE", None),
            account_key: String::new(),
            account_label: None,
            plan: None,
            five_hour: None,
            weekly: None,
            buckets: vec![],
            reset_credits: None,
            credits: None,
            error: None,
            latency_ms: None,
            usage: None,
            usage_status: "UNAVAILABLE".into(),
            next_attempt_at: None,
        }
    }
}
pub fn timestamp(v: &Value) -> Option<i64> {
    let n = v.as_i64()?;
    if n <= 0 {
        return None;
    }
    Some(if n > 100_000_000_000 { n / 1000 } else { n })
}
fn window(v: &Value) -> Option<Window> {
    let used = v["usedPercent"].as_f64()?;
    if !used.is_finite() {
        return None;
    }
    let used = used.clamp(0.0, 100.0);
    Some(Window {
        used_percent: used,
        remaining_percent: 100.0 - used,
        window_minutes: v["windowDurationMins"].as_u64(),
        resets_at: timestamp(&v["resetsAt"]),
    })
}
pub fn normalize(v: &Value, at: i64) -> Result<Quota> {
    let snapshots = v["rateLimitsByLimitId"].as_object();
    let selected = snapshots
        .and_then(|s| s.get("codex"))
        .unwrap_or(&v["rateLimits"]);
    anyhow::ensure!(
        selected.is_object(),
        "No account rate-limit object returned"
    );
    let pair = [window(&selected["primary"]), window(&selected["secondary"])];
    let mut buckets = vec![];
    if let Some(map) = snapshots {
        for (id, b) in map {
            buckets.push(Bucket {
                id: id.to_owned(),
                name: b["limitName"].as_str().map(str::to_owned),
                primary: window(&b["primary"]),
                secondary: window(&b["secondary"]),
            });
        }
    } else {
        buckets.push(Bucket {
            id: selected["limitId"].as_str().unwrap_or("codex").into(),
            name: None,
            primary: pair[0].clone(),
            secondary: pair[1].clone(),
        });
    }
    let reset = &v["rateLimitResetCredits"];
    let reset_credits = reset["availableCount"]
        .as_u64()
        .map(|available_count| ResetCredits {
            available_count,
            details: reset["credits"].as_array().map(|items| {
                items
                    .iter()
                    .map(|c| ResetCredit {
                        status: c["status"].as_str().map(str::to_owned),
                        reset_type: c["resetType"].as_str().map(str::to_owned),
                        granted_at: timestamp(&c["grantedAt"]),
                        expires_at: timestamp(&c["expiresAt"]),
                        title: c["title"].as_str().map(str::to_owned),
                    })
                    .collect()
            }),
        });
    Ok(Quota {
        meta: Provenance::new(
            "codex app-server · account/rateLimits/read",
            "LIVE",
            Some(at),
        ),
        five_hour: pair
            .iter()
            .flatten()
            .find(|w| w.window_minutes == Some(300))
            .cloned(),
        weekly: pair
            .iter()
            .flatten()
            .find(|w| w.window_minutes == Some(10080))
            .cloned(),
        buckets,
        reset_credits,
        plan: selected["planType"].as_str().map(str::to_owned),
        credits: selected.get("credits").filter(|v| v.is_object()).cloned(),
        ..Quota::default()
    })
}
pub fn executable() -> Result<PathBuf> {
    let mut list = vec![];
    if let Some(p) = std::env::var_os("CODEXMETER_CODEX_BINARY") {
        list.push(PathBuf::from(p));
    }
    for p in [
        "/Applications/ChatGPT.app/Contents/Resources/codex",
        "/Applications/Codex.app/Contents/Resources/codex",
        "/opt/homebrew/bin/codex",
        "/usr/local/bin/codex",
    ] {
        list.push(p.into());
    }
    if let Some(h) = dirs::home_dir() {
        for p in [
            "Applications/ChatGPT.app/Contents/Resources/codex",
            "Applications/Codex.app/Contents/Resources/codex",
            ".local/bin/codex",
            ".cargo/bin/codex",
        ] {
            list.push(h.join(p));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        list.extend(std::env::split_paths(&path).map(|p| p.join("codex")));
    }
    list.into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("Install and sign in to official Codex or ChatGPT first"))
}
pub struct Client {
    child: Child,
    input: ChildStdin,
    rx: mpsc::Receiver<Value>,
    id: u64,
    pub executable: PathBuf,
}
impl Client {
    pub fn start() -> Result<Self> {
        Self::start_binary(executable()?)
    }
    pub fn start_binary(binary: PathBuf) -> Result<Self> {
        let mut child = Command::new(&binary)
            .args(["-c", "analytics.enabled=false", "app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Missing app-server input"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Missing app-server output"))?;
        let (tx, rx) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Ok((line, _, true, false)) = bounded_line(&mut reader, 8 * 1024 * 1024) {
                if let Ok(v) = serde_json::from_slice::<Value>(&line) {
                    if v.get("id").is_some() && tx.send(v).is_err() {
                        break;
                    }
                }
            }
        });
        let mut client = Self {
            child,
            input,
            rx,
            id: 0,
            executable: binary,
        };
        client.request("initialize",Some(json!({"clientInfo":{"name":"codex_unified_monitor","title":"Codex Unified Monitor","version":"0.1.0"},"capabilities":{"experimentalApi":true}})))?;
        writeln!(client.input, "{}", json!({"method":"initialized"}))?;
        client.input.flush()?;
        Ok(client)
    }
    pub fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        anyhow::ensure!(
            READ_METHODS.contains(&method),
            "Method is outside the read-only allowlist"
        );
        if method == "account/read" {
            anyhow::ensure!(
                params.as_ref().is_some_and(|p| p["refreshToken"] == false),
                "Token refresh is disabled"
            );
        }
        self.id += 1;
        let id = self.id;
        let mut m = json!({"id":id,"method":method});
        if let Some(p) = params {
            m["params"] = p;
        }
        writeln!(self.input, "{m}")?;
        self.input.flush()?;
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            let wait = end
                .checked_duration_since(Instant::now())
                .ok_or_else(|| anyhow!("Account request timed out"))?;
            let v = self
                .rx
                .recv_timeout(wait)
                .map_err(|_| anyhow!("Account request timed out or disconnected"))?;
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(e) = v.get("error") {
                // Do not surface upstream error strings or stderr, which may contain account secrets.
                let code = e["code"].as_i64().unwrap_or(0);
                let message = e["message"].as_str().unwrap_or("").to_lowercase();
                if code == 429 || message.contains("429") || message.contains("too many") {
                    return Err(anyhow!("Rate limited (429); backing off"));
                }
                if code == -32601 {
                    return Err(anyhow!("Unsupported account method"));
                }
                return Err(anyhow!(
                    "Account request failed (code {code}); check official Codex sign-in"
                ));
            }
            return v
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("Account response has no result"));
        }
    }
    pub fn read(&mut self, fetch_usage: bool) -> Result<Quota> {
        let begin = Instant::now();
        let a = self.request("account/read", Some(json!({"refreshToken":false})))?;
        let r = self.request("account/rateLimits/read", None)?;
        let mut q = normalize(&r, now())?;
        let acc = &a["account"];
        let identity = r["accountId"]
            .as_str()
            .or(acc["email"].as_str())
            .unwrap_or("unidentified");
        q.account_key = digest(identity);
        q.account_label = acc["email"].as_str().map(|mail| {
            let domain = mail.split('@').nth(1).unwrap_or("");
            format!("{}…@{domain}", mail.chars().next().unwrap_or('•'))
        });
        q.plan = q
            .plan
            .or_else(|| acc["planType"].as_str().map(str::to_owned));
        if fetch_usage {
            match self.request("account/usage/read", None) {
                Ok(u) => {
                    q.usage = Some(
                        json!({"summary":u["summary"],"dailyUsageBuckets":u["dailyUsageBuckets"]}),
                    );
                    q.usage_status = "LIVE".into();
                }
                Err(e) => {
                    q.usage_status = if e.to_string().contains("Unsupported") {
                        "UNSUPPORTED"
                    } else {
                        "UNAVAILABLE"
                    }
                    .into();
                }
            }
        }
        q.latency_ms = Some(begin.elapsed().as_millis());
        Ok(q)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn backoff(base: u64, failures: u32) -> u64 {
    base.clamp(60, 3600)
        .saturating_mul(2u64.saturating_pow(failures.min(6)))
        .min(3600)
}
pub fn save(store: &Store, q: &Quota) -> Result<()> {
    anyhow::ensure!(
        q.meta.status == "LIVE",
        "Only successful reads enter quota history"
    );
    store.conn.execute(
        "INSERT OR REPLACE INTO quota_snapshots(timestamp,account_key,payload) VALUES(?,?,?)",
        rusqlite::params![q.meta.updated_at, q.account_key, serde_json::to_string(q)?],
    )?;
    Ok(())
}
pub fn cached(store: &Store) -> Result<Quota> {
    let value: Option<String> = store
        .conn
        .query_row(
            "SELECT payload FROM quota_snapshots ORDER BY timestamp DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let mut q: Quota = value
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    if q.meta.updated_at.is_some() {
        q.meta.status = "CACHED".into();
        q.meta.confidence = "limited".into();
        q.usage_status = "CACHED".into();
    }
    Ok(q)
}
pub fn history(store: &Store, account: &str) -> Result<Vec<Quota>> {
    let mut s = store
        .conn
        .prepare("SELECT payload FROM quota_snapshots WHERE account_key=? ORDER BY timestamp")?;
    let texts = s
        .query_map([account], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    texts.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
}

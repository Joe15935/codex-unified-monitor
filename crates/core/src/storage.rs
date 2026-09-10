use crate::{accounting::Tokens, parser::Event};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub struct Store {
    pub conn: Connection,
    pub path: PathBuf,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        anyhow::ensure!(version <= 3, "database_newer_than_app");
        if version == 0 {
            conn.execute_batch("BEGIN;
              CREATE TABLE sessions(id TEXT PRIMARY KEY, project TEXT NOT NULL, started_at INTEGER NOT NULL);
              CREATE TABLE events(id TEXT PRIMARY KEY,response_id TEXT UNIQUE,session_id TEXT NOT NULL,timestamp INTEGER NOT NULL,model TEXT NOT NULL,effort TEXT NOT NULL,turn_id TEXT NOT NULL,kind TEXT NOT NULL,tool TEXT NOT NULL,raw_input INTEGER NOT NULL,cached_input INTEGER NOT NULL,output INTEGER NOT NULL,reasoning INTEGER NOT NULL,cache_write INTEGER NOT NULL,source TEXT NOT NULL);
              CREATE INDEX events_time ON events(timestamp); CREATE INDEX events_session ON events(session_id,timestamp);
              CREATE TABLE files(id INTEGER PRIMARY KEY,path TEXT UNIQUE NOT NULL,device INTEGER NOT NULL,inode INTEGER NOT NULL,mtime TEXT NOT NULL,size INTEGER NOT NULL,offset INTEGER NOT NULL,head_len INTEGER NOT NULL,head_hash TEXT NOT NULL,tail_hash TEXT NOT NULL,parser_state TEXT NOT NULL,errors INTEGER NOT NULL DEFAULT 0,oversized INTEGER NOT NULL DEFAULT 0);
              CREATE INDEX files_inode ON files(device,inode);
              CREATE TABLE event_files(file_id INTEGER NOT NULL REFERENCES files(id),event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,PRIMARY KEY(file_id,event_id));
              CREATE TABLE quota_snapshots(timestamp INTEGER NOT NULL,account_key TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(timestamp,account_key));
              CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
              CREATE TABLE pricing_catalog(id INTEGER PRIMARY KEY CHECK(id=1),payload TEXT NOT NULL);
              CREATE TABLE diagnostics(key TEXT PRIMARY KEY,value INTEGER NOT NULL);
              PRAGMA user_version=2; COMMIT;")?;
        }
        if version == 1 {
            // Rebuild only this app's derived index for the corrected thread /
            // fork accounting. Settings, quota history and original logs stay.
            conn.execute_batch(
                "BEGIN; DELETE FROM event_files; DELETE FROM events;
                DELETE FROM files; DELETE FROM sessions; DELETE FROM diagnostics;
                PRAGMA user_version=2; COMMIT;",
            )?;
        }
        if version < 3 {
            // Additive migration: existing counters, file offsets and history survive.
            conn.execute_batch("BEGIN;
              ALTER TABLE events ADD COLUMN service_tier TEXT;
              ALTER TABLE events ADD COLUMN is_subagent INTEGER;
              CREATE TABLE audit_runs(id INTEGER PRIMARY KEY,account_key TEXT NOT NULL,started_at INTEGER NOT NULL,ended_at INTEGER,config TEXT NOT NULL);
              CREATE TABLE audit_origins(account_key TEXT PRIMARY KEY,origin TEXT NOT NULL);
              PRAGMA user_version=3; COMMIT;")?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self {
            conn,
            path: path.to_owned(),
        })
    }
    pub fn events(&self, start: i64, end: i64) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        self.visit_events(start, end, |e| events.push(e))?;
        Ok(events)
    }
    pub fn visit_events(&self, start: i64, end: i64, mut visit: impl FnMut(Event)) -> Result<()> {
        let mut stmt=self.conn.prepare("SELECT id,response_id,session_id,timestamp,model,effort,turn_id,kind,tool,raw_input,cached_input,output,reasoning,cache_write,source,service_tier,is_subagent FROM events WHERE timestamp>=? AND timestamp<? ORDER BY timestamp,id")?;
        let rows = stmt.query_map(params![start, end], |r| {
            let raw: u64 = r.get(9)?;
            let cached: u64 = r.get(10)?;
            let output: u64 = r.get(11)?;
            Ok(Event {
                id: r.get(0)?,
                response_id: r.get(1)?,
                session_id: r.get(2)?,
                timestamp: r.get(3)?,
                model: r.get(4)?,
                effort: r.get(5)?,
                turn_id: r.get(6)?,
                kind: r.get(7)?,
                tool: r.get(8)?,
                tokens: Tokens {
                    raw_input_tokens: raw,
                    cached_input_tokens: cached,
                    uncached_input_tokens: raw - cached,
                    output_tokens: output,
                    reasoning_output_tokens: r.get(12)?,
                    total_tokens: raw + output,
                    cache_write_input_tokens: r.get(13)?,
                },
                source: r.get(14)?,
                service_tier: r.get(15)?,
                is_subagent: r.get(16)?,
            })
        })?;
        for row in rows {
            visit(row?);
        }
        Ok(())
    }
    pub fn increment(&self, key: &str, value: u64) -> Result<()> {
        self.conn.execute("INSERT INTO diagnostics(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=value+excluded.value",params![key,value])?;
        Ok(())
    }
}

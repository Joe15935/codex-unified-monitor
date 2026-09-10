//! SQLite-backed incremental port of CodexScope's manifest/store design.
use crate::{
    digest,
    parser::{self, ParserState},
    storage::Store,
};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{
    fs::{File, Metadata},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const MAX_LINE: usize = 2 * 1024 * 1024;
#[derive(Debug, Default, Serialize)]
pub struct IngestResult {
    pub files_seen: u64,
    pub files_parsed: u64,
    pub bytes_read: u64,
    pub inserted: u64,
    pub duplicates: u64,
    pub errors: u64,
    pub oversized: u64,
    pub elapsed_ms: u128,
}
#[derive(Default)]
struct Checkpoint {
    id: i64,
    path: String,
    mtime: String,
    size: u64,
    offset: u64,
    head_len: u64,
    head_hash: String,
    tail_hash: String,
    state: String,
    errors: u64,
    oversized: u64,
}

/// Bounds allocation independently of the physical line length. An incomplete final line is not committed.
pub fn bounded_line<R: BufRead>(
    reader: &mut R,
    limit: usize,
) -> std::io::Result<(Vec<u8>, u64, bool, bool)> {
    let mut line = Vec::new();
    let mut bytes = 0;
    let mut oversized = false;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok((line, bytes, false, oversized));
        }
        let newline = buf.iter().position(|b| *b == b'\n');
        let n = newline.map(|p| p + 1).unwrap_or(buf.len());
        bytes += n as u64;
        if line.len() + n <= limit && !oversized {
            line.extend_from_slice(&buf[..n]);
        } else {
            oversized = true;
            line.clear();
        }
        reader.consume(n);
        if newline.is_some() {
            return Ok((line, bytes, true, oversized));
        }
    }
}
fn fingerprint(path: &Path, start: u64, len: u64) -> Result<String> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(start))?;
    let mut bytes = vec![0; len as usize];
    f.read_exact(&mut bytes)?;
    Ok(digest(bytes))
}
fn identity(meta: &Metadata) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        (0, 0)
    }
}
pub fn discover(home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in [home.join("sessions"), home.join("archived_sessions")] {
        for e in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "jsonl") {
                paths.push(e.path().to_owned());
            }
        }
    }
    paths.sort();
    paths
}
pub fn ingest(store: &mut Store, paths: &[PathBuf]) -> Result<IngestResult> {
    let start = std::time::Instant::now();
    let mut result = IngestResult::default();
    for path in paths {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        result.files_seen += 1;
        ingest_file(store, path, &meta, &mut result)?;
    }
    store.increment("duplicates", result.duplicates)?;
    store.increment("bytes_read", result.bytes_read)?;
    result.elapsed_ms = start.elapsed().as_millis();
    Ok(result)
}
fn ingest_file(
    store: &mut Store,
    path: &Path,
    meta: &Metadata,
    result: &mut IngestResult,
) -> Result<()> {
    let key = path.to_string_lossy().to_string();
    let (device, inode) = identity(meta);
    let mtime = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    let size = meta.len();
    let old=store.conn.query_row("SELECT id,path,mtime,size,offset,head_len,head_hash,tail_hash,parser_state,errors,oversized FROM files WHERE path=? OR (?!=0 AND device=? AND inode=?) ORDER BY path=? DESC LIMIT 1",params![key,inode,device,inode,key],|r|Ok(Checkpoint{id:r.get(0)?,path:r.get(1)?,mtime:r.get(2)?,size:r.get(3)?,offset:r.get(4)?,head_len:r.get(5)?,head_hash:r.get(6)?,tail_hash:r.get(7)?,state:r.get(8)?,errors:r.get(9)?,oversized:r.get(10)?})).optional()?;
    let mut old = old.unwrap_or_default();
    if old.id != 0 && old.mtime == mtime && old.size == size {
        if old.path != key {
            store
                .conn
                .execute("UPDATE files SET path=? WHERE id=?", params![key, old.id])?;
        }
        return Ok(());
    }
    let mut reset = old.id == 0 || size < old.offset;
    if old.id != 0 && !reset {
        reset = (size <= old.size && mtime != old.mtime)
            || fingerprint(path, 0, old.head_len).ok().as_deref() != Some(old.head_hash.as_str())
            || fingerprint(path, old.offset.saturating_sub(256), old.offset.min(256))
                .ok()
                .as_deref()
                != Some(old.tail_hash.as_str());
    }
    let tx = store.conn.transaction()?;
    if old.id == 0 {
        tx.execute("INSERT INTO files(path,device,inode,mtime,size,offset,head_len,head_hash,tail_hash,parser_state) VALUES(?,?,?,?,?,0,0,'','','{}')",params![key,device,inode,mtime,size])?;
        old.id = tx.last_insert_rowid();
    } else if reset {
        tx.execute("DELETE FROM event_files WHERE file_id=?", [old.id])?;
        tx.execute("DELETE FROM events WHERE NOT EXISTS(SELECT 1 FROM event_files WHERE event_id=events.id)",[])?;
    }
    let fallback = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown");
    let mut state = if reset {
        ParserState::default()
    } else {
        serde_json::from_str(&old.state)?
    };
    if state.session_id.is_empty() {
        state.session_id = fallback
            .chars()
            .rev()
            .take(36)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
    }
    let mut offset = if reset { 0 } else { old.offset };
    let mut errors = if reset { 0 } else { old.errors };
    let mut oversized = if reset { 0 } else { old.oversized };
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut reader = BufReader::with_capacity(64 * 1024, file.take(size - offset));
    result.files_parsed += 1;
    loop {
        let (line, bytes, complete, too_large) = bounded_line(&mut reader, MAX_LINE)?;
        result.bytes_read += bytes;
        if !complete {
            break;
        }
        offset += bytes;
        if too_large {
            oversized += 1;
            result.oversized += 1;
            continue;
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match parser::parse(&line, &mut state) {
            Ok(event) => {
                tx.execute("INSERT INTO sessions(id,project,started_at) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET project=CASE WHEN excluded.project='' THEN sessions.project ELSE excluded.project END,started_at=CASE WHEN sessions.started_at=0 THEN excluded.started_at WHEN excluded.started_at=0 THEN sessions.started_at ELSE MIN(sessions.started_at,excluded.started_at) END",params![state.session_id,state.project,state.started_at])?;
                if let Some(e) = event {
                    let existing:Option<String>=tx.query_row("SELECT id FROM events WHERE id=? OR (? IS NOT NULL AND response_id=?) LIMIT 1",params![e.id,e.response_id,e.response_id],|r|r.get(0)).optional()?;
                    let id = existing.as_deref().unwrap_or(&e.id);
                    if existing.is_some() {
                        result.duplicates += 1;
                    }
                    let t = e.tokens;
                    let changed=tx.execute("INSERT INTO events(id,response_id,session_id,timestamp,model,effort,turn_id,kind,tool,raw_input,cached_input,output,reasoning,cache_write,source) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET response_id=excluded.response_id,source=excluded.source WHERE excluded.source='token_usage_record' AND events.source!='token_usage_record'",params![id,e.response_id,e.session_id,e.timestamp,e.model,e.effort,e.turn_id,e.kind,e.tool,t.raw_input_tokens,t.cached_input_tokens,t.output_tokens,t.reasoning_output_tokens,t.cache_write_input_tokens,e.source])?;
                    if existing.is_none() {
                        result.inserted += changed as u64;
                    }
                    tx.execute(
                        "INSERT OR IGNORE INTO event_files(file_id,event_id) VALUES(?,?)",
                        params![old.id, id],
                    )?;
                }
            }
            Err(_) => {
                errors += 1;
                result.errors += 1;
            }
        }
    }
    let head_len = size.min(4096);
    let head_hash = fingerprint(path, 0, head_len)?;
    let tail_hash = fingerprint(path, offset.saturating_sub(256), offset.min(256))?;
    tx.execute("UPDATE files SET path=?,device=?,inode=?,mtime=?,size=?,offset=?,head_len=?,head_hash=?,tail_hash=?,parser_state=?,errors=?,oversized=? WHERE id=?",params![key,device,inode,mtime,size,offset,head_len,head_hash,tail_hash,serde_json::to_string(&state)?,errors,oversized,old.id])?;
    tx.commit()?;
    Ok(())
}

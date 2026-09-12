//! Scheduling and recovery rules; no provider, timer, or database ownership.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Default)]
pub struct IngestActivity {
    pub bytes_read: u64,
    pub inserted: u64,
    pub index_changed: bool,
    pub complete: bool,
}

/// Normal ingestion is one batch. Only after an error do we isolate individual
/// paths, so an unreadable file cannot block unrelated sessions indefinitely.
/// Checkpoints make replay of any successful prefix safe.
pub fn ingest_pending(
    store: &mut codexmeter_core::storage::Store,
    pending: &mut BTreeSet<PathBuf>,
) -> IngestActivity {
    let batch: Vec<_> = pending.iter().cloned().collect();
    if let Ok(result) = codexmeter_core::ingest::ingest(store, &batch) {
        pending.clear();
        return IngestActivity {
            bytes_read: result.bytes_read,
            inserted: result.inserted,
            index_changed: result.files_parsed > 0,
            complete: true,
        };
    }
    // The first error may follow a committed prefix; its health transition also
    // needs a UI update. Later retries use per-file results directly instead of
    // SQLite total_changes, which includes rolled-back writes.
    let mut activity = retry_pending(store, pending);
    activity.index_changed = true;
    activity
}

pub fn retry_pending(
    store: &mut codexmeter_core::storage::Store,
    pending: &mut BTreeSet<PathBuf>,
) -> IngestActivity {
    let batch: Vec<_> = pending.iter().cloned().collect();
    let mut activity = IngestActivity::default();
    for path in batch {
        if let Ok(result) = codexmeter_core::ingest::ingest(store, std::slice::from_ref(&path)) {
            pending.remove(&path);
            activity.bytes_read = activity.bytes_read.saturating_add(result.bytes_read);
            activity.inserted = activity.inserted.saturating_add(result.inserted);
            activity.index_changed |= result.files_parsed > 0;
        }
    }
    activity.complete = pending.is_empty();
    activity
}

pub const FALLBACK_RECONCILE_SECONDS: i64 = 60;
pub const NATIVE_RECONCILE_SECONDS: i64 = 900;
pub const WATCH_RETRY_SECONDS: i64 = 300;
pub const WATCH_CHECK_SECONDS: i64 = 60;
pub const MANUAL_REFRESH_FLOOR_SECONDS: i64 = 15;

#[derive(Debug)]
pub struct RefreshPolicy {
    last_activity: i64,
}

impl RefreshPolicy {
    pub fn new(now: i64) -> Self {
        Self { last_activity: now }
    }

    /// Metadata-only reconciliation and duplicated watcher events are not activity.
    pub fn observe_ingest(&mut self, now: i64, bytes_read: u64, inserted: u64) {
        if bytes_read > 0 || inserted > 0 {
            self.last_activity = now;
        }
    }

    pub fn interval(&self, now: i64, configured: u64, adaptive: bool, audit_active: bool) -> u64 {
        if !adaptive || audit_active {
            return configured;
        }
        let idle = now.saturating_sub(self.last_activity);
        let factor = if idle >= 900 {
            4
        } else if idle >= 300 {
            2
        } else {
            1
        };
        // Never poll more often than configured, including custom intervals > 5 min.
        configured.saturating_mul(factor).min(configured.max(300))
    }
}

pub fn next_attempt(last_attempt: i64, interval: u64, retry_until: i64, manual: bool) -> i64 {
    let delay = if manual {
        MANUAL_REFRESH_FLOOR_SECONDS
    } else {
        interval.min(i64::MAX as u64) as i64
    };
    last_attempt.saturating_add(delay).max(retry_until)
}

pub fn freshness_seconds(configured: u64, adaptive: bool) -> i64 {
    let base = configured.min(i64::MAX as u64 / 3) as i64;
    if adaptive {
        (base * 3).max(base.max(300) + base)
    } else {
        base * 3
    }
}

#[derive(Debug, Default)]
pub struct RecoveryPolicy {
    pub native_ready: bool,
    retry_watch_at: i64,
    ingest_failures: u32,
    pub retry_ingest_at: i64,
}

impl RecoveryPolicy {
    pub fn watch_failed(&mut self, now: i64) {
        self.native_ready = false;
        self.retry_watch_at = now.saturating_add(WATCH_RETRY_SECONDS);
    }
    pub fn watch_attached(&mut self) {
        self.native_ready = true;
        self.retry_watch_at = 0;
    }
    pub fn roots_changed(&mut self) {
        self.native_ready = false;
        self.retry_watch_at = 0;
    }
    pub fn may_attach(&self, now: i64) -> bool {
        !self.native_ready && now >= self.retry_watch_at
    }
    pub fn reconcile_seconds(&self) -> i64 {
        if self.native_ready {
            NATIVE_RECONCILE_SECONDS
        } else {
            FALLBACK_RECONCILE_SECONDS
        }
    }
    pub fn ingest_failed(&mut self, now: i64) {
        self.ingest_failures = self.ingest_failures.saturating_add(1);
        let delay = (5i64 << self.ingest_failures.saturating_sub(1).min(4)).min(60);
        self.retry_ingest_at = now.saturating_add(delay);
    }
    pub fn ingest_succeeded(&mut self) {
        self.ingest_failures = 0;
        self.retry_ingest_at = 0;
    }
    pub fn ingest_is_healthy(&self) -> bool {
        self.ingest_failures == 0
    }
    pub fn status(&self) -> &'static str {
        if self.ingest_failures > 0 {
            "ingest error; retrying automatically"
        } else if self.native_ready {
            "watching"
        } else {
            "watcher degraded; 60-second reconciliation"
        }
    }
}

pub fn existing_roots(home: &Path) -> Vec<PathBuf> {
    [home.join("sessions"), home.join("archived_sessions")]
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}

/// kqueue opens a descriptor per node, including non-log files and directories.
/// Stop counting at the budget and reject links: notify follows links recursively.
/// This bounds the native attempt; fallback discovery never follows symlinks.
pub fn tree_within_budget(roots: &[PathBuf], budget: usize) -> bool {
    let mut pending: Vec<_> = roots.to_vec();
    let mut remaining = budget;
    while let Some(path) = pending.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            return false;
        };
        if remaining == 0 || meta.file_type().is_symlink() {
            return false;
        }
        remaining -= 1;
        if meta.is_dir() {
            let Ok(entries) = std::fs::read_dir(&path) else {
                return false;
            };
            for entry in entries {
                let Ok(entry) = entry else { return false };
                // Bound temporary storage as well as the number of stat calls.
                if pending.len() >= remaining {
                    return false;
                }
                pending.push(entry.path());
            }
        }
    }
    true
}

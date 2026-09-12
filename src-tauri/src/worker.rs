use crate::{
    native_ui::{self, ChangedText},
    refresh_policy::{self, RecoveryPolicy, RefreshPolicy},
    AppState, Message,
};
use codexmeter_core::{
    account::{self, Client},
    analytics::{bounds, Aggregate, Range},
    ingest, now,
    settings::Settings,
};
use notify::Watcher;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use tauri::{Emitter, Manager};

// kqueue opens a descriptor for every node. Reserve room for SQLite, the provider,
// WebKit and the rest of the app rather than changing the user's process limits.
#[cfg(target_os = "macos")]
const NATIVE_NODE_BUDGET: usize = 128;

fn attach_watcher(
    home: &Path,
    roots: &[PathBuf],
    tx: mpsc::Sender<Message>,
) -> Option<(notify::RecommendedWatcher, Arc<AtomicBool>)> {
    if !home.is_dir() || roots.is_empty() {
        return None;
    }
    let failed = Arc::new(AtomicBool::new(false));
    let callback_failed = failed.clone();
    let watched_home = home.to_owned();
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            match event {
                Ok(e) if !matches!(e.kind, notify::EventKind::Access(_)) => {
                    let paths: Vec<_> = e
                        .paths
                        .into_iter()
                        .filter(|p| {
                            p == &watched_home
                                || p.starts_with(watched_home.join("sessions"))
                                || p.starts_with(watched_home.join("archived_sessions"))
                        })
                        .collect();
                    if !paths.is_empty() {
                        let _ = tx.send(Message::Files(paths));
                    }
                }
                Err(_) => {
                    // Do not expose paths or OS error bodies; wake the owner to drop
                    // all partial registrations and switch to metadata reconciliation.
                    callback_failed.store(true, Ordering::Release);
                    let _ = tx.send(Message::Files(vec![]));
                }
                _ => {}
            }
        })
        .ok()?;
    // Do not watch CODEX_HOME itself: notify 6 kqueue can recursively register
    // new children even on a nominally non-recursive parent registration.
    // Missing roots are discovered by the bounded root existence check instead.
    for root in roots {
        // Only register the two roots; notify handles their descendants itself.
        // A failed registration drops the whole local watcher on return.
        watcher.watch(root, notify::RecursiveMode::Recursive).ok()?;
    }
    Some((watcher, failed))
}

pub fn run(app: tauri::AppHandle, rx: mpsc::Receiver<Message>) {
    let state = app.state::<AppState>();
    let home = codexmeter_core::codex_home();
    let mut watcher: Option<(notify::RecommendedWatcher, Arc<AtomicBool>)> = None;
    let mut observed_roots: Option<Vec<PathBuf>> = None;
    let mut recovery = RecoveryPolicy::default();
    let mut policy = RefreshPolicy::new(now());
    let mut next_watch_check = 0;
    let mut reconcile = now() + refresh_policy::FALLBACK_RECONCILE_SECONDS;
    let mut paths: BTreeSet<PathBuf> = ingest::discover(&home).into_iter().collect();
    let mut client: Option<Client> = None;
    let mut retry_until = 0;
    let mut last_attempt = 0;
    let mut failures = 0;
    let mut last_usage = 0;
    let mut manual_requested = false;
    let mut previously_enabled = None;
    let mut initial_scan = true;
    let mut tray_text = (ChangedText::default(), ChangedText::default());
    loop {
        let time = now();
        if watcher
            .as_ref()
            .is_some_and(|(_, failed)| failed.load(Ordering::Acquire))
        {
            watcher = None;
            recovery.watch_failed(time);
            reconcile = reconcile.min(time + recovery.reconcile_seconds());
        }
        if time >= next_watch_check {
            let roots = refresh_policy::existing_roots(&home);
            if observed_roots.as_ref().is_some_and(|seen| seen != &roots) {
                watcher = None;
                recovery.roots_changed();
            }
            observed_roots = Some(roots.clone());
            #[cfg(target_os = "macos")]
            let safe_native = refresh_policy::tree_within_budget(&roots, NATIVE_NODE_BUDGET);
            #[cfg(not(target_os = "macos"))]
            let safe_native = true;
            if !safe_native {
                watcher = None;
                recovery.watch_failed(time);
                reconcile = reconcile.min(time + recovery.reconcile_seconds());
            } else if recovery.may_attach(time) {
                watcher = attach_watcher(&home, &roots, state.tx.clone());
                if watcher.is_some() {
                    recovery.watch_attached();
                    // Reconcile once after recovery; these are metadata checks,
                    // and unchanged files do not reopen the historical logs.
                    paths.extend(ingest::discover(&home));
                    reconcile = time + recovery.reconcile_seconds();
                } else {
                    recovery.watch_failed(time);
                    reconcile = reconcile.min(time + recovery.reconcile_seconds());
                }
            }
            next_watch_check = time + refresh_policy::WATCH_CHECK_SECONDS;
        }
        let mut ingested_at = (initial_scan && paths.is_empty()).then_some(time);
        let mut index_changed = initial_scan;
        initial_scan = false;
        if !paths.is_empty() && time >= recovery.retry_ingest_at {
            let recovering = !recovery.ingest_is_healthy();
            let result = state.store.lock().ok().map(|mut s| {
                if recovering {
                    refresh_policy::retry_pending(&mut s, &mut paths)
                } else {
                    refresh_policy::ingest_pending(&mut s, &mut paths)
                }
            });
            if let Some(result) = result {
                index_changed |= result.index_changed;
                policy.observe_ingest(now(), result.bytes_read, result.inserted);
                if result.complete {
                    recovery.ingest_succeeded();
                    ingested_at = Some(now());
                } else {
                    recovery.ingest_failed(now());
                }
            } else {
                // Preserve failed paths and retry soon; metadata checkpoints make
                // replay of the successful prefix safe and inexpensive.
                recovery.ingest_failed(now());
            }
        }
        let notify_local = if let Ok(mut local) = state.local.lock() {
            let status = recovery.status();
            let changed = native_ui::needs_local_notification(&local.0, status, index_changed);
            local.0 = status.into();
            if let Some(time) = ingested_at {
                local.1 = Some(time);
            }
            changed
        } else {
            false
        };
        if notify_local {
            // Never hold the status mutex while dispatching to UI listeners.
            let _ = app.emit("monitor-updated", ());
        }
        let (settings, audit_active) = state
            .store
            .lock()
            .ok()
            .map(|s| {
                let settings = Settings::load(&s).unwrap_or_default();
                // An unreadable control state must not reduce observation coverage.
                let active: bool = s
                    .conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM audit_runs WHERE ended_at IS NULL)",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(true);
                (settings, active)
            })
            .unwrap_or((Settings::default(), true));
        if previously_enabled == Some(false) && settings.account_enabled {
            manual_requested = true;
        }
        previously_enabled = Some(settings.account_enabled);
        let interval = policy.interval(
            now(),
            settings.quota_poll_seconds,
            settings.adaptive_refresh,
            audit_active || !recovery.ingest_is_healthy(),
        );
        let mut next_quota =
            refresh_policy::next_attempt(last_attempt, interval, retry_until, manual_requested);
        if !settings.account_enabled {
            client = None;
            manual_requested = false;
            if let Ok(mut q) = state.quota.lock() {
                q.meta.status = "UNAVAILABLE".into();
                q.error = Some("Account connection disabled in Settings".into());
                q.meta.confidence = "limited".into();
                q.next_attempt_at = None;
            }
            next_quota = now() + refresh_policy::WATCH_CHECK_SECONDS;
        } else if now() >= next_quota {
            last_attempt = now();
            manual_requested = false;
            let read_usage = last_usage == 0 || now() - last_usage >= 900;
            let result = (|| {
                if client.is_none() {
                    client = Some(Client::start()?);
                }
                client.as_mut().unwrap().read(read_usage)
            })();
            match result {
                Ok(mut q) => {
                    failures = 0;
                    retry_until = 0;
                    next_quota = refresh_policy::next_attempt(last_attempt, interval, 0, false);
                    q.next_attempt_at = Some(next_quota);
                    if let Ok(mut current) = state.quota.lock() {
                        if !read_usage && current.account_key == q.account_key {
                            q.usage = current.usage.clone();
                            q.usage_status = "CACHED".into();
                        }
                        if read_usage {
                            last_usage = now();
                        }
                        *current = q.clone();
                    }
                    if let Ok(store) = state.store.lock() {
                        let _ = account::save(&store, &q);
                    }
                }
                Err(e) => {
                    failures += 1;
                    retry_until =
                        now() + account::backoff(settings.quota_poll_seconds, failures) as i64;
                    next_quota = retry_until;
                    if let Ok(mut q) = state.quota.lock() {
                        q.meta.status = if q.meta.updated_at.is_some() {
                            "STALE"
                        } else {
                            "UNAVAILABLE"
                        }
                        .into();
                        q.meta.confidence = "limited".into();
                        q.error = Some(e.to_string());
                        q.next_attempt_at = Some(next_quota);
                    }
                    // Drop kills and reaps only this component's owned child.
                    client = None;
                }
            }
            let _ = app.emit("monitor-updated", ());
        } else if let Ok(mut q) = state.quota.lock() {
            q.next_attempt_at = Some(next_quota);
        }
        update_tray(&app, &settings, &mut tray_text);
        let retry_ingest = if paths.is_empty() {
            i64::MAX
        } else {
            recovery.retry_ingest_at
        };
        let deadline = next_quota
            .min(reconcile)
            .min(next_watch_check)
            .min(retry_ingest);
        let seconds = (deadline - now()).clamp(1, 60) as u64;
        let mut messages = vec![];
        match rx.recv_timeout(Duration::from_secs(seconds)) {
            Ok(m) => {
                messages.push(m);
                std::thread::sleep(Duration::from_millis(250));
                messages.extend(rx.try_iter().take(1024));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for m in messages {
            match m {
                Message::Quit => return,
                Message::Refresh => {
                    paths.extend(ingest::discover(&home));
                    recovery.retry_ingest_at = 0;
                    manual_requested = true;
                    next_watch_check = 0;
                }
                Message::Settings => {
                    // Language, theme and pricing changes must not generate extra
                    // account requests or perturb controlled sampling intervals.
                    let _ = app.emit("monitor-updated", ());
                }
                Message::Files(list) => {
                    for path in list {
                        if path == home
                            || path == home.join("sessions")
                            || path == home.join("archived_sessions")
                        {
                            next_watch_check = 0;
                        }
                        if path == home {
                            // Parent notifications only signal possible root creation.
                            // Unrelated CODEX_HOME writes must not rescan both trees.
                            continue;
                        } else if path.is_dir() {
                            next_watch_check = 0;
                            paths.extend(walk_rollouts(&path));
                        } else if path.extension().is_some_and(|e| e == "jsonl") {
                            paths.insert(path);
                        }
                    }
                }
            }
        }
        if now() >= reconcile {
            paths.extend(ingest::discover(&home));
            reconcile = now() + recovery.reconcile_seconds();
        }
    }
}
fn walk_rollouts(path: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = vec![];
    if !std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()) {
        return paths;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return paths;
    };
    for e in entries.flatten() {
        let p = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            paths.extend(walk_rollouts(&p));
        } else if e.file_type().is_ok_and(|t| t.is_file())
            && p.extension().is_some_and(|x| x == "jsonl")
        {
            paths.push(p);
        }
    }
    paths
}
fn update_tray(app: &tauri::AppHandle, settings: &Settings, sent: &mut (ChangedText, ChangedText)) {
    let st = app.state::<AppState>();
    let Ok(q) = st.quota.lock() else { return };
    let live = q.meta.status == "LIVE"
        && q.meta.updated_at.is_some_and(|t| {
            (0..=refresh_policy::freshness_seconds(
                settings.quota_poll_seconds,
                settings.adaptive_refresh,
            ))
                .contains(&(now() - t))
        });
    let label = match settings.tray_metric.as_str() {
        "weekly" => {
            if live {
                q.weekly
                    .as_ref()
                    .map(|w| format!("C {:.0}%", w.remaining_percent))
            } else {
                None
            }
        }
        "five_hour" => {
            if live {
                q.five_hour
                    .as_ref()
                    .map(|w| format!("C {:.0}%", w.remaining_percent))
            } else {
                None
            }
        }
        _ => None,
    };
    let status = if q.meta.status == "LIVE" && !live {
        "STALE"
    } else {
        &q.meta.status
    }
    .to_owned();
    drop(q);
    let label = if settings.tray_metric.starts_with("today_") {
        st.store.lock().ok().and_then(|store| {
            let tz = settings.timezone.parse::<chrono_tz::Tz>().ok()?;
            let (start, end) = bounds(&Range::default(), tz, now()).ok()?;
            let c = crate::catalog(&store);
            let mut aggregate = Aggregate::default();
            store
                .visit_events(start, end, |e| aggregate.add(&e, &c, settings))
                .ok()?;
            Some(if settings.tray_metric == "today_tokens" {
                format!("C {:.1}M", aggregate.tokens.total_tokens as f64 / 1e6)
            } else {
                aggregate
                    .money(&settings.pricing_mode)
                    .value()
                    .map(|v| format!("C ${v:.2}"))
                    .unwrap_or_else(|| {
                        format!(
                            "C {}",
                            codexmeter_core::locale::text(&settings.language, "unpriced")
                        )
                    })
            })
        })
    } else {
        label
    };
    if let Some(tray) = app.tray_by_id("monitor") {
        let label = label.unwrap_or("C —".into());
        let metric = match settings.tray_metric.as_str() {
            "five_hour" => "5-hour remaining",
            "today_tokens" => "Today tokens",
            "today_value" => "Today API equivalent",
            _ => "Weekly remaining",
        };
        let tip = format!(
            "Codex Unified Monitor · {} · {} · {}",
            codexmeter_core::locale::text(&settings.language, metric),
            label.trim_start_matches("C "),
            codexmeter_core::locale::text(&settings.language, &status)
        );
        // Windows ignores tray titles; its tooltip still includes the selected value.
        #[cfg(not(target_os = "windows"))]
        let _ = sent.0.deliver(label, |title| tray.set_title(Some(title)));
        let _ = sent
            .1
            .deliver(tip, |tooltip| tray.set_tooltip(Some(tooltip)));
    }
}

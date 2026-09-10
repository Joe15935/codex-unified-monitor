use crate::{AppState, Message};
use codexmeter_core::{
    account::{self, Client},
    analytics::{bounds, Aggregate, Range},
    ingest, now,
    settings::Settings,
};
use notify::Watcher;
use std::{collections::BTreeSet, path::PathBuf, sync::mpsc, time::Duration};
use tauri::{Emitter, Manager};

pub fn run(app: tauri::AppHandle, rx: mpsc::Receiver<Message>) {
    let state = app.state::<AppState>();
    let home = codexmeter_core::codex_home();
    let tx = state.tx.clone();
    let watched_home = home.clone();
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            if let Ok(e) = event {
                if matches!(e.kind, notify::EventKind::Access(_)) {
                    return;
                }
                let paths: Vec<_> = e
                    .paths
                    .into_iter()
                    .filter(|p| {
                        p.starts_with(watched_home.join("sessions"))
                            || p.starts_with(watched_home.join("archived_sessions"))
                    })
                    .collect();
                if !paths.is_empty() {
                    let _ = tx.send(Message::Files(paths));
                }
            }
        })
        .ok();
    let watching = watcher.as_mut().is_some_and(|w| {
        // kqueue observes the two log trees directly. Watching all of CODEX_HOME
        // recursively would waste descriptors on plugin caches and other data.
        let mut ok = w.watch(&home, notify::RecursiveMode::NonRecursive).is_ok();
        for root in [home.join("sessions"), home.join("archived_sessions")] {
            if root.is_dir() {
                ok &= w.watch(&root, notify::RecursiveMode::Recursive).is_ok();
            }
        }
        ok
    });
    let mut paths = ingest::discover(&home);
    let mut client: Option<Client> = None;
    let mut next_quota = 0;
    let mut retry_until = 0;
    let mut last_attempt = 0;
    let mut failures = 0;
    let mut last_usage = 0;
    let mut reconcile = now() + 900;
    loop {
        if !paths.is_empty() {
            let result = state
                .store
                .lock()
                .ok()
                .and_then(|mut s| ingest::ingest(&mut s, &paths).ok());
            if let Ok(mut local) = state.local.lock() {
                *local = (
                    if result.is_some() {
                        if watching {
                            "watching"
                        } else {
                            "watcher unavailable; 15-minute reconciliation"
                        }
                    } else {
                        "ingest error; retry with Refresh"
                    }
                    .into(),
                    Some(now()),
                );
            }
            paths.clear();
            let _ = app.emit("monitor-updated", ());
        } else if let Ok(mut local) = state.local.lock() {
            if local.0 == "scanning" {
                *local = (
                    if watching {
                        "watching"
                    } else {
                        "watcher unavailable"
                    }
                    .into(),
                    Some(now()),
                );
            }
        }
        let settings = state
            .store
            .lock()
            .ok()
            .and_then(|s| Settings::load(&s).ok())
            .unwrap_or_default();
        if !settings.account_enabled {
            client = None;
            if let Ok(mut q) = state.quota.lock() {
                q.meta.status = "UNAVAILABLE".into();
                q.error = Some("Account connection disabled in Settings".into());
                q.meta.confidence = "limited".into();
            }
            next_quota = now() + settings.quota_poll_seconds as i64;
        } else if now() >= next_quota && now() >= retry_until {
            last_attempt = now();
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
                    next_quota = now() + settings.quota_poll_seconds as i64;
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
                    // Drop kills and reaps the owned child before any later reconnect.
                    client = None;
                }
            }
            let _ = app.emit("monitor-updated", ());
        }
        update_tray(&app, &settings);
        let seconds = (next_quota.min(reconcile) - now()).clamp(1, 60) as u64;
        let mut messages = vec![];
        if let Ok(m) = rx.recv_timeout(Duration::from_secs(seconds)) {
            messages.push(m);
            std::thread::sleep(Duration::from_millis(250));
            messages.extend(rx.try_iter());
        }
        let mut selected = BTreeSet::<PathBuf>::new();
        for m in messages {
            match m {
                Message::Quit => return,
                Message::Refresh => {
                    paths = ingest::discover(&home);
                    if now() >= retry_until && now() - last_attempt >= 15 {
                        next_quota = 0;
                    }
                }
                Message::Settings => {
                    if now() >= retry_until {
                        next_quota = 0;
                    }
                    let _ = app.emit("monitor-updated", ());
                }
                Message::Files(list) => {
                    for path in list {
                        if path.is_dir() {
                            if let Some(w) = watcher.as_mut() {
                                let _ = w.watch(&path, notify::RecursiveMode::Recursive);
                            }
                            for entry in walk_rollouts(&path) {
                                selected.insert(entry);
                            }
                        } else if path.extension().is_some_and(|e| e == "jsonl") {
                            selected.insert(path);
                        }
                    }
                }
            }
        }
        paths.extend(selected);
        if now() >= reconcile {
            paths = ingest::discover(&home);
            reconcile = now() + 900;
        }
    }
}
fn walk_rollouts(path: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = vec![];
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
fn update_tray(app: &tauri::AppHandle, settings: &Settings) {
    let st = app.state::<AppState>();
    let Ok(q) = st.quota.lock() else { return };
    let live = q.meta.status == "LIVE"
        && q.meta
            .updated_at
            .is_some_and(|t| now() - t <= settings.quota_poll_seconds as i64 * 3);
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
    let tip = format!(
        "Codex Unified Monitor · {} · remaining quota",
        q.meta.status
    );
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
                    .unwrap_or("C unpriced".into())
            })
        })
    } else {
        label
    };
    if let Some(tray) = app.tray_by_id("monitor") {
        let _ = tray.set_title(Some(label.unwrap_or("C —".into())));
        let _ = tray.set_tooltip(Some(tip));
    }
}

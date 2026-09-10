mod tray;
mod worker;
use codexmeter_core::{
    account::{self, Quota},
    analytics::{self, Range},
    pricing::Catalog,
    settings::Settings,
    storage::Store,
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as _;

pub enum Message {
    Files(Vec<PathBuf>),
    Refresh,
    Settings,
    Quit,
}
pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub quota: Arc<Mutex<Quota>>,
    pub local: Arc<Mutex<(String, Option<i64>)>>,
    pub tx: mpsc::Sender<Message>,
    pub worker_join: Mutex<Option<std::thread::JoinHandle<()>>>,
}
fn catalog(store: &Store) -> Catalog {
    store
        .conn
        .query_row("SELECT payload FROM pricing_catalog WHERE id=1", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Catalog::bundled)
}
#[tauri::command]
async fn dashboard(app: tauri::AppHandle, range: Option<Range>) -> Result<Value, String> {
    let st = app.state::<AppState>();
    let store = st.store.clone();
    let quota = st.quota.clone();
    let local = st.local.clone();
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move||{
  let store=store.lock().map_err(|_|"Database lock unavailable")?;let settings=Settings::load(&store).map_err(|e|e.to_string())?;let c=catalog(&store);
  let mut r=analytics::report(&store,&c,&settings,range.unwrap_or_default(),codexmeter_core::now()).map_err(|e|e.to_string())?;
  let mut q=quota.lock().map_err(|_|"Quota lock unavailable")?.clone();
  if q.meta.status=="LIVE"&&q.meta.updated_at.is_some_and(|t|codexmeter_core::now()-t>settings.quota_poll_seconds as i64*3){q.meta.status="STALE".into();q.meta.confidence="limited".into();}
  let state=local.lock().map_err(|_|"Local status unavailable")?;r.diagnostics.scan_status=state.0.clone();r.diagnostics.last_local_update=state.1;r.meta.updated_at=state.1;
  let history=account::history(&store,&q.account_key).map_err(|e|e.to_string())?;
  let five=analytics::burn(&history,"5h",&store,&c,&settings).map_err(|e|e.to_string())?;let week=analytics::burn(&history,"week",&store,&c,&settings).map_err(|e|e.to_string())?;
  Ok(json!({"report":r,"quota":q,"settings":settings,"burn":{"five_hour":five,"weekly":week},"autostart":autostart,"version":"0.1.0"}))
 }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
fn refresh(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .tx
        .send(Message::Refresh)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let state = app.state::<AppState>();
    settings
        .save(
            &*state
                .store
                .lock()
                .map_err(|_| "Database lock unavailable")?,
        )
        .map_err(|e| e.to_string())?;
    state
        .tx
        .send(Message::Settings)
        .map_err(|e| e.to_string())?;
    app.emit("monitor-updated", ()).map_err(|e| e.to_string())
}
#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|e| e.to_string())?;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}
#[tauri::command]
fn open_dashboard(app: tauri::AppHandle, tab: Option<String>) {
    tray::show_dashboard(&app);
    if let Some(tab) = tab {
        let _ = app.emit_to("main", "navigate", tab);
    }
}
#[tauri::command]
async fn session_detail(app: tauri::AppHandle, id: String) -> Result<Value, String> {
    let store = app.state::<AppState>().store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock().map_err(|_| "Database lock unavailable")?;
        let s = Settings::load(&store).map_err(|e| e.to_string())?;
        serde_json::to_value(
            analytics::session_detail(&store, &id, &catalog(&store), &s)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn export_report(
    app: tauri::AppHandle,
    range: Range,
    format: String,
    dataset: String,
) -> Result<String, String> {
    let store = app.state::<AppState>().store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock().map_err(|_| "Database lock unavailable")?;
        let s = Settings::load(&store).map_err(|e| e.to_string())?;
        let mut r = analytics::report(&store, &catalog(&store), &s, range, codexmeter_core::now())
            .map_err(|e| e.to_string())?;
        if dataset == "daily" {
            use chrono::TimeZone;
            let tz = s
                .timezone
                .parse::<chrono_tz::Tz>()
                .map_err(|e| e.to_string())?;
            let range = Range {
                period: "custom".into(),
                start: Some(
                    tz.timestamp_opt(r.start, 0)
                        .unwrap()
                        .format("%Y-%m-%d")
                        .to_string(),
                ),
                end: Some(
                    tz.timestamp_opt(r.end - 1, 0)
                        .unwrap()
                        .format("%Y-%m-%d")
                        .to_string(),
                ),
            };
            r = analytics::report(&store, &catalog(&store), &s, range, codexmeter_core::now())
                .map_err(|e| e.to_string())?;
        }
        let q = account::cached(&store).map_err(|e| e.to_string())?;
        let history = account::history(&store, &q.account_key).map_err(|e| e.to_string())?;
        let content = codexmeter_core::export::render(&r, &history, &format, &dataset)
            .map_err(|e| e.to_string())?;
        let dir = codexmeter_core::data_dir().join("exports");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!(
            "codex-{}-{}.{}",
            dataset,
            chrono::Utc::now().format("%Y%m%dT%H%M%S%3f"),
            format
        ));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_exports() -> Result<(), String> {
    let path = codexmeter_core::data_dir().join("exports");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    std::process::Command::new("/usr/bin/open")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn import_catalog(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let c: Catalog = serde_json::from_str(&text).map_err(|_| "Invalid pricing catalog JSON")?;
    if c.schema_version != 1 || c.entries.is_empty() {
        return Err("Unsupported or empty catalog".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for p in &c.entries {
        p.validate().map_err(|e| e.to_string())?;
        if !seen.insert(format!("{}:{}", p.pricing_mode, p.model)) {
            return Err("Duplicate model and pricing mode".into());
        }
    }
    let state = app.state::<AppState>();
    let store = state
        .store
        .lock()
        .map_err(|_| "Database lock unavailable")?;
    store.conn.execute("INSERT INTO pricing_catalog(id,payload) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(&c).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    app.emit("monitor-updated", ()).map_err(|e| e.to_string())
}
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    let _ = app.state::<AppState>().tx.send(Message::Quit);
    app.exit(0);
}
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show_dashboard(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .setup(|app| {
            let dir = codexmeter_core::data_dir();
            std::fs::create_dir_all(&dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let store = Store::open(&dir.join("monitor.sqlite3"))?;
            let cached = account::cached(&store)?;
            let (tx, rx) = mpsc::channel();
            app.manage(AppState {
                store: Arc::new(Mutex::new(store)),
                quota: Arc::new(Mutex::new(cached)),
                local: Arc::new(Mutex::new(("scanning".into(), None))),
                tx,
                worker_join: Mutex::new(None),
            });
            tray::install(app.handle())?;
            for label in ["main", "compact"] {
                if let Some(w) = app.get_webview_window(label) {
                    let window = w.clone();
                    let compact = label == "compact";
                    w.on_window_event(move |e| match e {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                        tauri::WindowEvent::Focused(false) if compact => {
                            let _ = window.hide();
                        }
                        _ => {}
                    });
                }
            }
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if std::env::args().any(|s| s == "--background") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            let handle = app.handle().clone();
            let join = std::thread::spawn(move || worker::run(handle, rx));
            *app.state::<AppState>().worker_join.lock().unwrap() = Some(join);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dashboard,
            refresh,
            save_settings,
            set_autostart,
            open_dashboard,
            session_detail,
            export_report,
            open_exports,
            import_catalog,
            quit
        ])
        .build(tauri::generate_context!())
        .expect("Unable to start Codex Unified Monitor");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            let state = app.state::<AppState>();
            let _ = state.tx.send(Message::Quit);
            if let Ok(mut guard) = state.worker_join.lock() {
                if let Some(join) = guard.take() {
                    let _ = join.join();
                }
            };
        }
    });
}

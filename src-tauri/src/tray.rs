//! Tray position adaptation from poer2023/CodexScope (HduSy MIT lineage).
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
pub fn show_dashboard(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if let Some(w) = app.get_webview_window("compact") {
        let _ = w.hide();
    }
}
pub fn install(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        "Quit Codex Unified Monitor",
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(app, &[&open, &refresh, &settings, &quit])?;
    TrayIconBuilder::with_id("monitor")
        .icon(tauri::include_image!("icons/tray-icon.png"))
        .icon_as_template(true)
        .title("C —")
        .tooltip("Codex Unified Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_dashboard(app),
            "settings" => {
                show_dashboard(app);
                let _ = app.emit_to("main", "navigate", "settings");
            }
            "refresh" => {
                let _ = app
                    .state::<crate::AppState>()
                    .tx
                    .send(crate::Message::Refresh);
            }
            "quit" => {
                let _ = app.state::<crate::AppState>().tx.send(crate::Message::Quit);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("compact") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                        return;
                    }
                    let size = w.outer_size().unwrap_or(tauri::PhysicalSize::new(370, 470));
                    let rect_position = rect.position.to_physical::<f64>(1.0);
                    let rect_size = rect.size.to_physical::<f64>(1.0);
                    let ax = rect_position.x + rect_size.width / 2.0;
                    let mut x = ax - size.width as f64 / 2.0;
                    let mut y = rect_position.y + rect_size.height;
                    let monitor = w
                        .available_monitors()
                        .ok()
                        .and_then(|ms| {
                            ms.into_iter().find(|m| {
                                ax >= m.position().x as f64
                                    && ax < (m.position().x as f64 + m.size().width as f64)
                            })
                        })
                        .or_else(|| w.monitor_from_point(position.x, position.y).ok().flatten());
                    if let Some(m) = monitor {
                        let scale = m.scale_factor();
                        let pos = m.position();
                        let margin = 8.0 * scale;
                        let minx = pos.x as f64 + margin;
                        let maxx =
                            pos.x as f64 + m.size().width as f64 - size.width as f64 - margin;
                        x = x.clamp(minx, maxx.max(minx));
                        let safe = pos.y as f64 + 42.0 * scale;
                        let maxy =
                            pos.y as f64 + m.size().height as f64 - size.height as f64 - margin;
                        if y < safe || y > safe + 24.0 * scale {
                            y = safe
                        }
                        y = y.clamp(safe, maxy.max(safe));
                    }
                    let _ = w.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                    let _ = w.show();
                    let _ = w.set_focus();
                    let _ = app.emit_to("compact", "monitor-updated", ());
                }
            }
        })
        .build(app)?;
    Ok(())
}

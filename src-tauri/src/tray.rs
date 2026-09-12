//! Tray position adaptation from poer2023/CodexScope (HduSy MIT lineage).
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
struct TrayLabels([MenuItem<tauri::Wry>; 4]);
const MENU_LABELS: [&str; 4] = [
    "Open Dashboard",
    "Refresh",
    "Settings",
    "Quit Codex Unified Monitor",
];

#[cfg(any(not(target_os = "macos"), test))]
#[derive(Clone, Copy)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(not(target_os = "macos"), test))]
fn panel_position(
    icon: PhysicalBounds,
    panel: (f64, f64),
    work: PhysicalBounds,
    scale: f64,
) -> (i32, i32) {
    let margin = 8.0 * scale;
    let min_x = work.x + margin;
    let max_x = (work.x + work.width - panel.0 - margin).max(min_x);
    let min_y = work.y + margin;
    let max_y = (work.y + work.height - panel.1 - margin).max(min_y);
    let x = (icon.x + icon.width / 2.0 - panel.0 / 2.0).clamp(min_x, max_x);
    let below = icon.y + icon.height + margin;
    let above = icon.y - panel.1 - margin;
    let y = if below <= max_y { below } else { above }.clamp(min_y, max_y);
    (x.round() as i32, y.round() as i32)
}

pub fn set_language(app: &tauri::AppHandle, language: &str) -> tauri::Result<()> {
    if let Some(items) = app.try_state::<TrayLabels>() {
        for (item, source) in items.inner().0.iter().zip(MENU_LABELS) {
            item.set_text(codexmeter_core::locale::text(language, source))?;
        }
    }
    Ok(())
}
pub fn show_dashboard(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        notify_visible(app, "main");
    }
    if let Some(w) = app.get_webview_window("compact") {
        let _ = w.hide();
    }
}
pub fn notify_visible(app: &tauri::AppHandle, label: &str) {
    let _ = app.emit_to(label, "monitor-visible", ());
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
    app.manage(TrayLabels([open, refresh, settings, quit]));
    let language = app
        .state::<crate::AppState>()
        .store
        .lock()
        .ok()
        .and_then(|store| codexmeter_core::settings::Settings::load(&store).ok())
        .map(|settings| settings.language)
        .unwrap_or("zh-CN".into());
    set_language(app, &language)?;
    TrayIconBuilder::with_id("monitor")
        .icon(tauri::include_image!("icons/tray-icon.png"))
        .icon_as_template(cfg!(target_os = "macos"))
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
                    let monitor = w
                        .monitor_from_point(position.x, position.y)
                        .ok()
                        .flatten()
                        .or_else(|| {
                            w.available_monitors().ok().and_then(|ms| {
                                ms.into_iter().find(|m| {
                                    position.x >= m.position().x as f64
                                        && position.x
                                            < (m.position().x as f64 + m.size().width as f64)
                                        && position.y >= m.position().y as f64
                                        && position.y
                                            < (m.position().y as f64 + m.size().height as f64)
                                })
                            })
                        });
                    let scale = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
                    let rect_position = rect.position.to_physical::<f64>(scale);
                    let rect_size = rect.size.to_physical::<f64>(scale);
                    let ax = rect_position.x + rect_size.width / 2.0;
                    let mut x = ax - size.width as f64 / 2.0;
                    let mut y = rect_position.y + rect_size.height;
                    if let Some(m) = monitor {
                        #[cfg(target_os = "macos")]
                        {
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
                        #[cfg(not(target_os = "macos"))]
                        {
                            let work = m.work_area();
                            let point = panel_position(
                                PhysicalBounds {
                                    x: rect_position.x,
                                    y: rect_position.y,
                                    width: rect_size.width,
                                    height: rect_size.height,
                                },
                                (size.width as f64, size.height as f64),
                                PhysicalBounds {
                                    x: work.position.x as f64,
                                    y: work.position.y as f64,
                                    width: work.size.width as f64,
                                    height: work.size.height as f64,
                                },
                                scale,
                            );
                            x = point.0 as f64;
                            y = point.1 as f64;
                        }
                    }
                    let _ = w.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                    let _ = w.show();
                    let _ = w.set_focus();
                    notify_visible(app, "compact");
                }
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: f64, y: f64, width: f64, height: f64) -> PhysicalBounds {
        PhysicalBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn bottom_taskbar_places_panel_above_the_icon_in_work_area() {
        let (x, y) = panel_position(
            rect(1850.0, 1040.0, 24.0, 40.0),
            (370.0, 470.0),
            rect(0.0, 0.0, 1920.0, 1040.0),
            1.0,
        );
        assert_eq!((x, y), (1542, 562));
        assert!(y + 470 < 1040);
    }

    #[test]
    fn top_taskbar_places_panel_below_the_icon() {
        assert_eq!(
            panel_position(
                rect(1800.0, 0.0, 24.0, 40.0),
                (370.0, 470.0),
                rect(0.0, 40.0, 1920.0, 1040.0),
                1.0
            ),
            (1542, 48)
        );
    }

    #[test]
    fn negative_monitor_coordinates_and_high_dpi_use_physical_units() {
        let (x, y) = panel_position(
            rect(-60.0, -80.0, 48.0, 80.0),
            (740.0, 940.0),
            rect(-2560.0, -1440.0, 2560.0, 1360.0),
            2.0,
        );
        assert_eq!((x, y), (-756, -1036));
    }

    #[test]
    fn side_taskbar_and_small_work_area_never_panic_or_cross_reserved_edge() {
        let (x, y) = panel_position(
            rect(0.0, 300.0, 48.0, 24.0),
            (370.0, 470.0),
            rect(48.0, 0.0, 1232.0, 1024.0),
            1.0,
        );
        assert_eq!((x, y), (56, 332));
        assert_eq!(
            panel_position(
                rect(180.0, 180.0, 24.0, 24.0),
                (370.0, 470.0),
                rect(0.0, 0.0, 200.0, 200.0),
                1.0
            ),
            (8, 8)
        );
    }
}

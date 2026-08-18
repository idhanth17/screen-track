// Screen Track desktop app: system-tray icon + dashboard window, with the
// capture loop and loopback ingest server embedded in-process.
// Always use the Windows GUI subsystem so no console window is allocated
// (even for debug builds); this is a tray app with no terminal UI.
#![windows_subsystem = "windows"]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

use screentrack_core::{run, store};

struct AppState {
    db_path: String,
}

fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tauri::command]
fn overview(date: Option<String>, state: tauri::State<AppState>) -> Result<store::Overview, String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    run::overview_for(&conn, date.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn week(date: Option<String>, state: tauri::State<AppState>) -> Result<Vec<store::DaySummary>, String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    store::week_summary(&conn, &date.unwrap_or_default(), 7).map_err(|e| e.to_string())
}

#[tauri::command]
fn correct(
    key: String,
    category: String,
    date: Option<String>,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    let d = date.filter(|s| !s.is_empty()).unwrap_or_else(today_str);
    let now = chrono::Local::now().timestamp_millis();
    store::set_entity_category(&conn, &d, &key, &category, now).map_err(|e| e.to_string())
}

fn main() {
    let db_path = run::default_db_path().to_string_lossy().to_string();
    let running = Arc::new(AtomicBool::new(true));

    // Embed the engine: capture loop + loopback ingest server.
    {
        let (r, d) = (running.clone(), db_path.clone());
        std::thread::spawn(move || {
            if let Err(e) = run::capture_loop(&d, r) {
                eprintln!("capture error: {e}");
            }
        });
    }
    {
        let (r, d) = (running.clone(), db_path.clone());
        std::thread::spawn(move || {
            if let Err(e) = run::serve_ingest(d, run::INGEST_PORT, r) {
                eprintln!("server error: {e}");
            }
        });
    }

    tauri::Builder::default()
        .manage(AppState { db_path })
        .invoke_handler(tauri::generate_handler![overview, week, correct])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show dashboard", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Screen Track", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Screen Track")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        // Closing the window hides to the tray instead of quitting.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Screen Track");
}

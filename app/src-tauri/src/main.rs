// Screen Track desktop app: system-tray icon + dashboard window, with the
// capture loop and loopback ingest server embedded in-process.
// Always use the Windows GUI subsystem so no console window is allocated
// (even for debug builds); this is a tray app with no terminal UI.
#![windows_subsystem = "windows"]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
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

/// Marker file that records the app has completed its first launch, so we only
/// auto-enable start-at-login once (and never fight a user who turned it off).
fn first_run_marker() -> std::path::PathBuf {
    run::data_dir().join(".first_run_done")
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

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<store::Settings, String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    store::get_settings(&conn).map_err(|e| e.to_string())
}

/// Whether the browser extension has reported in recently (live indicator).
#[tauri::command]
fn extension_status(state: tauri::State<AppState>) -> Result<store::ExtensionStatus, String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    Ok(store::extension_status(&conn, chrono::Local::now().timestamp_millis()))
}

/// Open a URL in the user's default browser (used by the "Download extension" button).
#[tauri::command]
fn open_url(url: String) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}

/// Fully uninstall Screen Track: turn off start-at-login, then (after the app
/// exits) run the installer's uninstaller, wipe all local data, and remove
/// shortcuts. Everything is deleted permanently.
#[tauri::command]
fn uninstall_app(app: tauri::AppHandle) {
    let _ = app.autolaunch().disable();
    let data_dir = run::data_dir();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let install_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let uninstaller = install_dir.map(|d| d.join("uninstall.exe"));
        let desktop = std::env::var("USERPROFILE").ok().map(|h| format!("{h}\\Desktop\\Screen Track.lnk"));

        // Detached script: wait for us to exit, run the uninstaller, wipe data + shortcuts.
        let mut ps = String::from("Start-Sleep -Seconds 2; ");
        if let Some(u) = &uninstaller {
            let u = u.display();
            ps.push_str(&format!("if (Test-Path '{u}') {{ Start-Process '{u}' -ArgumentList '/S' -Wait }}; "));
        }
        ps.push_str(&format!("Remove-Item -Recurse -Force '{}' -ErrorAction SilentlyContinue; ", data_dir.display()));
        if let Some(d) = &desktop {
            ps.push_str(&format!("Remove-Item -Force '{d}' -ErrorAction SilentlyContinue; "));
        }
        ps.push_str("Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'Screen Track' -ErrorAction SilentlyContinue;");

        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let sh = format!(
            "sleep 2; rm -rf '/Applications/Screen Track.app'; rm -rf '{}'; \
             osascript -e 'tell application \"System Events\" to delete login item \"Screen Track\"' 2>/dev/null || true",
            data_dir.display()
        );
        let _ = std::process::Command::new("sh").arg("-c").arg(&sh).spawn();
    }

    app.exit(0);
}

#[tauri::command]
fn set_settings(settings: store::Settings, state: tauri::State<AppState>) -> Result<(), String> {
    let conn = store::open(&state.db_path).map_err(|e| e.to_string())?;
    store::set_settings(&conn, &settings).map_err(|e| e.to_string())
}

fn main() {
    let db_path = run::default_db_path().to_string_lossy().to_string();
    let running = Arc::new(AtomicBool::new(true));

    // Embed the engine: capture loop + loopback ingest server. (AI enrichment is
    // spawned from `setup`, where the bundled model's resource path is known.)
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

    let enrich_running = running.clone();
    let enrich_db = db_path.clone();

    tauri::Builder::default()
        // Single-instance MUST be the first plugin. A second launch just focuses
        // the running app's window instead of starting a duplicate tracker.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        // Autostart-on-login. The registered command carries `--hidden` so a boot
        // launch comes up silent in the tray; a manual launch shows the window.
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        // Self-update: check GitHub Releases on launch; install a newer signed build.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState { db_path })
        .invoke_handler(tauri::generate_handler![
            overview, week, correct, get_settings, set_settings,
            extension_status, uninstall_app, open_url
        ])
        .setup(move |app| {
            // Self-update: check for a newer signed release and install it. Runs in
            // the background; on a boot launch the window is hidden so a restart is
            // invisible. Failures (offline, no update) are silent.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_updater::UpdaterExt;
                    if let Ok(updater) = handle.updater() {
                        if let Ok(Some(update)) = updater.check().await {
                            if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
                                handle.restart();
                            }
                        }
                    }
                });
            }

            // Spawn AI enrichment now that we can resolve the bundled model dir.
            // In a packaged app this is <resources>/minilm; a plain `cargo run`
            // without bundled resources simply won't find it and AI stays off.
            let model_dir = app.path().resource_dir().ok().map(|d| d.join("minilm"));
            {
                let (r, d) = (enrich_running.clone(), enrich_db.clone());
                std::thread::spawn(move || run::enrich_loop(&d, r, model_dir));
            }

            // Enable start-at-login exactly once (first ever run), so we respect
            // the user later turning it off from the tray menu.
            let marker = first_run_marker();
            if !marker.exists() {
                let _ = app.autolaunch().enable();
                let _ = std::fs::write(&marker, b"1");
            }
            let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

            let show = MenuItem::with_id(app, "show", "Show dashboard", true, None::<&str>)?;
            let startup = CheckMenuItem::with_id(
                app, "startup", "Start at login", true, autostart_on, None::<&str>,
            )?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Screen Track", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &startup, &sep, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Screen Track")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "startup" => {
                        let mgr = app.autolaunch();
                        let enabled = mgr.is_enabled().unwrap_or(false);
                        let _ = if enabled { mgr.disable() } else { mgr.enable() };
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

            // A boot/autostart launch passes `--hidden`: stay in the tray silently.
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
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

//! Long-running services: the capture loop and the loopback ingest server.
//! Both take a shared `running` flag so a host (daemon or Tauri app) can stop them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{Local, TimeZone};
use rusqlite::Connection;
use tiny_http::{Header, Method, Response, Server};

// The dashboard is embedded so the binary is self-contained.
const INDEX_HTML: &str = include_str!("../../app/dashboard/index.html");
const APP_CSS: &str = include_str!("../../app/dashboard/app.css");
const APP_JS: &str = include_str!("../../app/dashboard/app.js");

use crate::capture;
use crate::classify::classify_app;
use crate::model::{Category, IngestBody, Segment};
use crate::store;

const POLL: Duration = Duration::from_secs(3);
const IDLE_THRESHOLD_MS: u64 = 60_000;
const CHECKPOINT_MS: i64 = 30_000;

/// Default loopback port the extension posts browser activity to.
pub const INGEST_PORT: u16 = 47113;

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

/// Per-OS application data root:
/// Windows `%LOCALAPPDATA%`, macOS `~/Library/Application Support`,
/// Linux `$XDG_DATA_HOME` (or `~/.local/share`).
fn data_root() -> std::path::PathBuf {
    use std::path::PathBuf;
    #[cfg(windows)]
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library").join("Application Support");
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(p);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share");
        }
    }
    PathBuf::from(".")
}

/// The Screen Track data directory (created if missing).
pub fn data_dir() -> std::path::PathBuf {
    let dir = data_root().join("ScreenTrack");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// The default on-disk database location inside [`data_dir`].
pub fn default_db_path() -> std::path::PathBuf {
    data_dir().join("screentrack.sqlite")
}

/// Compute the unified overview for the current local day.
pub fn today_overview(conn: &Connection) -> Result<store::Overview> {
    let (from, to, date) = today_bounds();
    store::overview(conn, from, to, &date)
}

/// Overview for a given "YYYY-MM-DD" date, or today when `date` is None/empty.
pub fn overview_for(conn: &Connection, date: Option<&str>) -> Result<store::Overview> {
    match date {
        Some(d) if !d.is_empty() => {
            let (from, to) = store::date_bounds(d);
            store::overview(conn, from, to, d)
        }
        _ => today_overview(conn),
    }
}

fn query_param(raw: &str, key: &str) -> Option<String> {
    let q = raw.split_once('?')?.1;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some(key) {
            return it.next().map(|v| v.to_string());
        }
    }
    None
}

struct Current {
    app: String,
    category: Category,
    source: String,
    confidence: f32,
    idle: bool,
    start: i64,
}

fn resolve(conn: &Connection, app: &str) -> (Category, String, f32) {
    if let Ok(Some(cat)) = store::memory_lookup(conn, &format!("app:{}", app.to_lowercase())) {
        return (cat, "learned".to_string(), 0.98);
    }
    let (cat, conf, src) = classify_app(app);
    (cat, src.to_string(), conf)
}

fn flush(conn: &Connection, cur: &Current, end: i64) {
    if end <= cur.start {
        return;
    }
    let seg = Segment {
        start_ms: cur.start,
        end_ms: end,
        app: cur.app.clone(),
        category: cur.category,
        source: cur.source.clone(),
        confidence: cur.confidence,
        idle: cur.idle,
    };
    if let Err(e) = store::insert_segment(conn, &seg) {
        eprintln!("[warn] insert failed: {e}");
    }
}

/// Poll the foreground app + idle state, resolving activity into stored segments.
/// Blocks until `running` is cleared, then flushes the open segment.
pub fn capture_loop(db_path: &str, running: Arc<AtomicBool>) -> Result<()> {
    let conn = store::open(db_path)?;
    let mut cur: Option<Current> = None;

    while running.load(Ordering::SeqCst) {
        let now = now_ms();
        let snap = capture::snapshot();
        let app = snap
            .foreground
            .as_ref()
            .map(|f| f.process.clone())
            .unwrap_or_else(|| "(none)".to_string());
        let idle = snap.idle_ms >= IDLE_THRESHOLD_MS;
        let (category, source, confidence) = resolve(&conn, &app);

        match &mut cur {
            Some(c) if c.app == app && c.idle == idle && (now - c.start) < CHECKPOINT_MS => {}
            Some(c) => {
                flush(&conn, c, now);
                *c = Current { app, category, source, confidence, idle, start: now };
            }
            None => {
                cur = Some(Current { app, category, source, confidence, idle, start: now });
            }
        }

        let mut slept = Duration::ZERO;
        while slept < POLL && running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
            slept += Duration::from_millis(100);
        }
    }

    if let Some(c) = &cur {
        flush(&conn, c, now_ms());
    }
    Ok(())
}

/// Background AI enrichment: periodically embed unresolved channels/domains with
/// the bundled MiniLM model and cache the verdict, shrinking the review queue on
/// its own. The model is loaded once (it's ~90 MB); if no model directory is
/// available (e.g. a dev run without bundled resources) the loop simply exits and
/// the heuristics stand.
pub fn enrich_loop(db_path: &str, running: Arc<AtomicBool>, model_dir: Option<std::path::PathBuf>) {
    use crate::ai::Classifier;

    let conn = match store::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[warn] enrich: db open failed: {e}");
            return;
        }
    };

    let classifier = match model_dir {
        Some(dir) if dir.join("model.safetensors").exists() => match Classifier::load(&dir) {
            Ok(c) => {
                eprintln!("[ai] built-in model loaded from {}", dir.display());
                c
            }
            Err(e) => {
                eprintln!("[ai] model load failed ({e}); AI classification disabled");
                return;
            }
        },
        _ => {
            eprintln!("[ai] no bundled model found; AI classification disabled");
            return;
        }
    };

    // Gentle cadence: check for work about every 20s. Embedding is fast on CPU
    // and only runs when there's an unresolved entity.
    let mut wait = Duration::from_secs(18); // first pass shortly after startup
    while running.load(Ordering::SeqCst) {
        if wait >= Duration::from_secs(20) {
            wait = Duration::ZERO;
            if store::get_settings(&conn).unwrap_or_default().ai_enabled {
                if let Ok(cands) = store::entities_needing_ai(&conn, 8) {
                    for cand in cands {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Some(cat) = classifier.classify(&cand) {
                            let _ = store::learn_ai_category(&conn, &cand.key, cat.as_str(), now_ms());
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
        wait += Duration::from_millis(500);
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn cors(resp: Response<std::io::Empty>) -> Response<std::io::Empty> {
    resp.with_header(header("Access-Control-Allow-Origin", "*"))
}

fn today_bounds() -> (i64, i64, String) {
    let now = Local::now();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let start = Local.from_local_datetime(&midnight).unwrap().timestamp_millis();
    (start, start + 86_400_000, now.format("%Y-%m-%d").to_string())
}

fn serve_text(req: tiny_http::Request, body: &str, content_type: &str) {
    let r = Response::from_string(body)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Access-Control-Allow-Origin", "*"));
    let _ = req.respond(r);
}

/// Loopback HTTP server: accepts `POST /ingest` of the extension's browser
/// activity and writes it into the shared database. Never leaves the machine.
pub fn serve_ingest(db_path: String, port: u16, running: Arc<AtomicBool>) -> Result<()> {
    let server = match Server::http(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[warn] ingest server failed to bind :{port}: {e}");
            return Ok(());
        }
    };
    let conn = store::open(&db_path)?;

    while running.load(Ordering::SeqCst) {
        match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(req)) => handle(&conn, req),
            Ok(None) => {}
            Err(_) => {}
        }
    }
    Ok(())
}

fn handle(conn: &Connection, mut req: tiny_http::Request) {
    let method = req.method().clone();
    let raw = req.url().to_string();
    let url = raw.split('?').next().unwrap_or("/").to_string();

    match (&method, url.as_str()) {
        // CORS preflight for the extension's POST.
        (Method::Options, _) => {
            let r = Response::empty(204)
                .with_header(header("Access-Control-Allow-Origin", "*"))
                .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
                .with_header(header("Access-Control-Allow-Headers", "Content-Type"));
            let _ = req.respond(r);
        }

        // Dashboard assets.
        (Method::Get, "/") | (Method::Get, "/index.html") => {
            serve_text(req, INDEX_HTML, "text/html; charset=utf-8")
        }
        (Method::Get, "/app.css") => serve_text(req, APP_CSS, "text/css; charset=utf-8"),
        (Method::Get, "/app.js") => {
            serve_text(req, APP_JS, "application/javascript; charset=utf-8")
        }

        // Unified day view as JSON (optional ?date=YYYY-MM-DD).
        (Method::Get, "/overview") => {
            let date = query_param(&raw, "date");
            match overview_for(conn, date.as_deref()) {
                Ok(ov) => serve_text(req, &serde_json::to_string(&ov).unwrap_or_else(|_| "{}".into()), "application/json; charset=utf-8"),
                Err(e) => {
                    eprintln!("[warn] overview failed: {e}");
                    let _ = req.respond(cors(Response::empty(500)));
                }
            }
        }

        // Last 7 days of category totals (optional ?date= end date).
        (Method::Get, "/week") => {
            let end = query_param(&raw, "date").unwrap_or_else(|| today_bounds().2);
            match store::week_summary(conn, &end, 7) {
                Ok(w) => serve_text(req, &serde_json::to_string(&w).unwrap_or_else(|_| "[]".into()), "application/json; charset=utf-8"),
                Err(e) => {
                    eprintln!("[warn] week failed: {e}");
                    let _ = req.respond(cors(Response::empty(500)));
                }
            }
        }

        // Locked browser corrections for the extension to mirror.
        (Method::Get, "/corrections") => {
            match store::browser_corrections(conn) {
                Ok(pairs) => {
                    let map: std::collections::HashMap<String, String> = pairs.into_iter().collect();
                    serve_text(req, &serde_json::to_string(&map).unwrap_or_else(|_| "{}".into()), "application/json; charset=utf-8");
                }
                Err(_) => serve_text(req, "{}", "application/json; charset=utf-8"),
            }
        }

        // App settings (AI toggle + Ollama endpoint/model).
        (Method::Get, "/settings") => match store::get_settings(conn) {
            Ok(s) => serve_text(req, &serde_json::to_string(&s).unwrap_or_else(|_| "{}".into()), "application/json; charset=utf-8"),
            Err(_) => serve_text(req, "{}", "application/json; charset=utf-8"),
        },
        (Method::Post, "/settings") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                let _ = req.respond(cors(Response::empty(400)));
                return;
            }
            match serde_json::from_str::<store::Settings>(&body) {
                Ok(s) => {
                    if let Err(e) = store::set_settings(conn, &s) {
                        eprintln!("[warn] set_settings failed: {e}");
                    }
                    serve_text(req, "{\"ok\":true}", "application/json");
                }
                Err(_) => {
                    let _ = req.respond(cors(Response::empty(400)));
                }
            }
        }

        // Correction from the dashboard.
        (Method::Post, "/correct") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                let _ = req.respond(cors(Response::empty(400)));
                return;
            }
            match serde_json::from_str::<crate::model::CorrectBody>(&body) {
                Ok(c) => {
                    let date = if c.date.is_empty() { today_bounds().2 } else { c.date.clone() };
                    if let Err(e) = store::set_entity_category(conn, &date, &c.key, &c.category, now_ms()) {
                        eprintln!("[warn] correct failed: {e}");
                    }
                    serve_text(req, "{\"ok\":true}", "application/json");
                }
                Err(_) => {
                    let _ = req.respond(cors(Response::empty(400)));
                }
            }
        }

        // Browser activity ingest from the extension.
        (Method::Post, "/ingest") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                let _ = req.respond(cors(Response::empty(400)));
                return;
            }
            match serde_json::from_str::<IngestBody>(&body) {
                Ok(ib) => {
                    if let Err(e) = store::ingest_entities(conn, &ib.date, &ib.entities, now_ms()) {
                        eprintln!("[warn] ingest write failed: {e}");
                    }
                    serve_text(req, "{\"ok\":true}", "application/json");
                }
                Err(e) => {
                    let r = Response::from_string(format!("{{\"ok\":false,\"error\":\"{e}\"}}"))
                        .with_status_code(400)
                        .with_header(header("Access-Control-Allow-Origin", "*"));
                    let _ = req.respond(r);
                }
            }
        }

        _ => {
            let _ = req.respond(cors(Response::empty(404)));
        }
    }
}

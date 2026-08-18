//! screentrackd — the Screen Track capture daemon (headless CLI).
//!
//! Runs the OS capture loop and the loopback ingest server (so the browser
//! extension can push per-site/channel activity), and prints today's unified
//! totals. The Tauri app embeds the same services behind a dashboard.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{Local, TimeZone};

use screentrack_core::run::{self, INGEST_PORT};
use screentrack_core::store;

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

fn today() -> (i64, i64, String) {
    let now = Local::now();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let start = Local.from_local_datetime(&midnight).unwrap().timestamp_millis();
    (start, start + 86_400_000, now.format("%Y-%m-%d").to_string())
}

fn data_path() -> PathBuf {
    let mut dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    dir.push("ScreenTrack");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("screentrack.sqlite");
    dir
}

fn print_overview(conn: &rusqlite::Connection) {
    let (from, to, date) = today();
    match store::overview(conn, from, to, &date) {
        Ok(ov) if ov.total_ms > 0 => {
            println!("── today ({} min) ──", ov.total_ms / 60_000);
            for (cat, ms) in &ov.by_category {
                println!("   {:<14} {:>4} min", cat, ms / 60_000);
            }
            for a in ov.activities.iter().take(6) {
                println!("     · {:<22} {:<13} {} min", a.name, a.category, a.ms / 60_000);
            }
        }
        Ok(_) => println!("── today: nothing tracked yet ──"),
        Err(e) => eprintln!("[warn] overview failed: {e}"),
    }
}

fn main() -> Result<()> {
    let db = data_path();
    let db_str = db.to_string_lossy().to_string();
    println!("screentrackd: tracking → {}", db.display());
    println!("ingest server: http://127.0.0.1:{INGEST_PORT}/ingest");
    println!("(Ctrl-C to stop)\n");

    let running = Arc::new(AtomicBool::new(true));
    {
        let r = running.clone();
        ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))
            .expect("failed to set Ctrl-C handler");
    }

    let cap = {
        let (r, d) = (running.clone(), db_str.clone());
        std::thread::spawn(move || {
            if let Err(e) = run::capture_loop(&d, r) {
                eprintln!("capture error: {e}");
            }
        })
    };
    let srv = {
        let (r, d) = (running.clone(), db_str.clone());
        std::thread::spawn(move || {
            if let Err(e) = run::serve_ingest(d, INGEST_PORT, r) {
                eprintln!("server error: {e}");
            }
        })
    };

    let conn = store::open(&db_str)?;
    let mut last = now_ms();
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
        if now_ms() - last >= 30_000 {
            print_overview(&conn);
            last = now_ms();
        }
    }

    let _ = cap.join();
    let _ = srv.join();
    println!("\nstopped.");
    print_overview(&conn);
    Ok(())
}

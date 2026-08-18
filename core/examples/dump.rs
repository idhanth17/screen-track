//! Dev helper: dump the Screen Track DB — segments, browser entities, overview.
//! Run with: cargo run --example dump

use anyhow::Result;
use chrono::{Local, TimeZone};
use screentrack_core::store;

fn main() -> Result<()> {
    let mut p = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("ScreenTrack");
    p.push("screentrack.sqlite");

    let conn = store::open(p.to_string_lossy().as_ref())?;

    let now = Local::now();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let from = Local.from_local_datetime(&midnight).unwrap().timestamp_millis();
    let to = from + 86_400_000;
    let date = now.format("%Y-%m-%d").to_string();

    let ov = store::overview(&conn, from, to, &date)?;
    println!("=== OVERVIEW {} — {} min total ===", ov.date, ov.total_ms / 60_000);
    for (cat, ms) in &ov.by_category {
        println!("  {:<14} {} min", cat, ms / 60_000);
    }
    println!("\n  activities:");
    for a in &ov.activities {
        println!(
            "  [{:<7}] {:<24} {:<13} {}s{}",
            a.source, a.name, a.category, a.ms / 1000,
            if a.needs_review { "  (review)" } else { "" }
        );
    }
    Ok(())
}

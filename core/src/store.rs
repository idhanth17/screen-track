//! SQLite storage for sessions, activity segments, and learned category memory.

use crate::model::{BrowserEntity, Category, Segment};
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;

/// Local-midnight `[from, to)` millis for a "YYYY-MM-DD" date (today on parse error).
pub fn date_bounds(date: &str) -> (i64, i64) {
    let nd = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive());
    let midnight = nd.and_hms_opt(0, 0, 0).unwrap();
    let start = Local.from_local_datetime(&midnight).unwrap().timestamp_millis();
    (start, start + 86_400_000)
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS activity_segment (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    app        TEXT    NOT NULL,
    category   TEXT    NOT NULL,
    source     TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    idle       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_segment_time ON activity_segment(start_ms, end_ms);

CREATE TABLE IF NOT EXISTS category_memory (
    key          TEXT PRIMARY KEY,   -- "app:code.exe" | "domain:netflix.com" | "channel:UC.."
    display_name TEXT,
    category     TEXT NOT NULL,
    locked       INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL
);

-- Per-day browser activity pushed by the extension (sites + YouTube channels).
CREATE TABLE IF NOT EXISTS browser_entity_daily (
    date         TEXT NOT NULL,
    key          TEXT NOT NULL,
    kind         TEXT,
    name         TEXT,
    domain       TEXT,
    channel_id   TEXT,
    yt_category  TEXT,
    category     TEXT NOT NULL,
    ms           INTEGER NOT NULL,
    source       TEXT,
    needs_review INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (date, key)
);
"#;

/// Open (creating if needed) the database and ensure the schema exists.
pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Persist one resolved activity segment.
pub fn insert_segment(conn: &Connection, seg: &Segment) -> Result<()> {
    conn.execute(
        "INSERT INTO activity_segment (start_ms, end_ms, app, category, source, confidence, idle)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            seg.start_ms,
            seg.end_ms,
            seg.app,
            seg.category.as_str(),
            seg.source,
            seg.confidence,
            seg.idle as i64,
        ],
    )?;
    Ok(())
}

/// Active (non-idle) milliseconds per category within `[from_ms, to_ms)`.
pub fn totals(conn: &Connection, from_ms: i64, to_ms: i64) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT category, SUM(end_ms - start_ms) AS ms
         FROM activity_segment
         WHERE idle = 0 AND start_ms >= ?1 AND start_ms < ?2
         GROUP BY category
         ORDER BY ms DESC",
    )?;
    let rows = stmt
        .query_map(params![from_ms, to_ms], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Look up a locked/learned category override for a key (e.g. "app:code.exe").
pub fn memory_lookup(conn: &Connection, key: &str) -> Result<Option<Category>> {
    let mut stmt = conn.prepare("SELECT category FROM category_memory WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        let cat: String = row.get(0)?;
        Ok(Some(Category::from_str(&cat)))
    } else {
        Ok(None)
    }
}

/// Upsert a learned/corrected category for a key.
pub fn memory_set(
    conn: &Connection,
    key: &str,
    display_name: &str,
    category: Category,
    locked: bool,
    now_ms: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO category_memory (key, display_name, category, locked, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(key) DO UPDATE SET
            display_name = excluded.display_name,
            category     = excluded.category,
            locked       = excluded.locked,
            updated_at   = excluded.updated_at",
        params![key, display_name, category.as_str(), locked as i64, now_ms],
    )?;
    Ok(())
}

/// Upsert a batch of per-day browser entities (cumulative ms overwrites prior).
pub fn ingest_entities(
    conn: &Connection,
    date: &str,
    entities: &[BrowserEntity],
    now_ms: i64,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for e in entities {
        tx.execute(
            "INSERT INTO browser_entity_daily
               (date, key, kind, name, domain, channel_id, yt_category, category, ms, source, needs_review, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(date, key) DO UPDATE SET
               kind=excluded.kind, name=excluded.name, domain=excluded.domain,
               channel_id=excluded.channel_id, yt_category=excluded.yt_category,
               category=excluded.category, ms=excluded.ms, source=excluded.source,
               needs_review=excluded.needs_review, updated_at=excluded.updated_at",
            params![
                date, e.key, e.kind, e.name, e.domain, e.channel_id, e.yt_category,
                e.category, e.ms, e.source, e.needs_review as i64, now_ms
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// One row in the unified dashboard: a desktop app, a site, or a YouTube channel.
#[derive(Debug, Serialize)]
pub struct Activity {
    pub key: String,    // "app:code.exe" | "domain:.." | "channel:.." — for corrections
    pub source: String, // "app" | "site" | "youtube"
    pub name: String,
    pub category: String,
    pub ms: i64,
    pub needs_review: bool,
    pub detail: String, // domain, YouTube category, etc.
}

/// The unified day view the dashboard renders.
#[derive(Debug, Serialize)]
pub struct Overview {
    pub date: String,
    pub total_ms: i64,
    pub by_category: Vec<(String, i64)>,
    pub activities: Vec<Activity>,
}

/// Build the unified day view: native apps (excluding browsers, which the
/// extension breaks down per-site) + browser sites + YouTube channels.
pub fn overview(conn: &Connection, from_ms: i64, to_ms: i64, date: &str) -> Result<Overview> {
    use crate::classify::is_browser;

    let mut cat: HashMap<String, i64> = HashMap::new();
    let mut activities: Vec<Activity> = Vec::new();

    // Native apps — browsers excluded (the extension provides their real breakdown).
    {
        let mut stmt = conn.prepare(
            "SELECT app, category, SUM(end_ms - start_ms)
             FROM activity_segment
             WHERE idle = 0 AND start_ms >= ?1 AND start_ms < ?2
             GROUP BY app, category",
        )?;
        let rows = stmt.query_map(params![from_ms, to_ms], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        let self_apps = ["screen-track-app.exe", "screentrackd.exe"];
        for row in rows {
            let (app, category, ms) = row?;
            if app == "(none)" || is_browser(&app) || self_apps.contains(&app.to_lowercase().as_str()) {
                continue;
            }
            *cat.entry(category.clone()).or_insert(0) += ms;
            activities.push(Activity {
                key: format!("app:{}", app.to_lowercase()),
                source: "app".into(),
                name: app,
                category,
                ms,
                needs_review: false,
                detail: String::new(),
            });
        }
    }

    // Browser entities (sites + YouTube channels) for the day.
    {
        let mut stmt = conn.prepare(
            "SELECT key, kind, name, domain, yt_category, category, ms, needs_review
             FROM browser_entity_daily WHERE date = ?1",
        )?;
        let rows = stmt.query_map(params![date], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })?;
        for row in rows {
            let (key, kind, name, domain, yt, category, ms, nr) = row?;
            if matches!(domain.as_deref(), Some("127.0.0.1") | Some("localhost")) {
                continue;
            }
            *cat.entry(category.clone()).or_insert(0) += ms;
            let is_yt = kind.as_deref() == Some("youtube");
            activities.push(Activity {
                key,
                source: if is_yt { "youtube".into() } else { "site".into() },
                name,
                category,
                ms,
                needs_review: nr != 0,
                detail: if is_yt { yt.unwrap_or_default() } else { domain.unwrap_or_default() },
            });
        }
    }

    activities.sort_by(|a, b| b.ms.cmp(&a.ms));
    let mut by_category: Vec<(String, i64)> = cat.into_iter().collect();
    by_category.sort_by(|a, b| b.1.cmp(&a.1));
    let total_ms = by_category.iter().map(|(_, m)| m).sum();

    Ok(Overview { date: date.to_string(), total_ms, by_category, activities })
}

/// Correct an entity's category from the dashboard. Locks it in memory (so it
/// sticks) and retroactively re-tags today's data so the change shows at once:
/// app segments for an "app:" key, or the browser tally for a "domain:"/"channel:" key.
pub fn set_entity_category(conn: &Connection, date: &str, key: &str, category: &str, now_ms: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO category_memory (key, display_name, category, locked, updated_at)
         VALUES (?1, ?2, ?3, 1, ?4)
         ON CONFLICT(key) DO UPDATE SET category=excluded.category, locked=1, updated_at=excluded.updated_at",
        params![key, key, category, now_ms],
    )?;

    if let Some(app) = key.strip_prefix("app:") {
        let (from, to) = date_bounds(date);
        conn.execute(
            "UPDATE activity_segment SET category=?1 WHERE lower(app)=?2 AND start_ms>=?3 AND start_ms<?4",
            params![category, app.to_lowercase(), from, to],
        )?;
    } else {
        conn.execute(
            "UPDATE browser_entity_daily SET category=?1, source='manual', needs_review=0 WHERE date=?2 AND key=?3",
            params![category, date, key],
        )?;
    }
    Ok(())
}

/// Locked browser corrections (channels/domains) for the extension to pull and
/// mirror into its own memory — keeps dashboard and extension in agreement.
pub fn browser_corrections(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT key, category FROM category_memory
         WHERE locked = 1 AND (key LIKE 'channel:%' OR key LIKE 'domain:%')",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// One day's category totals for the weekly view.
#[derive(Debug, Serialize)]
pub struct DaySummary {
    pub date: String,
    pub total_ms: i64,
    pub by_category: Vec<(String, i64)>,
}

/// Category totals for each of the last `days` days ending at `end_date` (inclusive).
pub fn week_summary(conn: &Connection, end_date: &str, days: i64) -> Result<Vec<DaySummary>> {
    let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive());
    let mut out = Vec::new();
    for i in (0..days).rev() {
        let d = end - Duration::days(i);
        let ds = d.format("%Y-%m-%d").to_string();
        let (from, to) = date_bounds(&ds);
        let ov = overview(conn, from, to, &ds)?;
        out.push(DaySummary { date: ds, total_ms: ov.total_ms, by_category: ov.by_category });
    }
    Ok(out)
}

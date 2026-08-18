//! Screen Track core engine.
//!
//! Modules:
//! - [`model`]    — shared types (Category, Foreground, Segment)
//! - [`classify`] — auto-first classification (mirrors the browser extension)
//! - [`capture`]  — OS foreground-app + idle capture (per-platform)
//! - [`store`]    — SQLite storage + daily aggregation

pub mod model;
pub mod classify;
pub mod capture;
pub mod store;
pub mod run;
pub mod ai;

pub use model::{BrowserEntity, Category, Foreground, Segment};

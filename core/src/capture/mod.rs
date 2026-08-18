//! OS capture: current foreground app + input-idle time.
//! Platform implementations live behind a `cfg`; the daemon uses [`snapshot`].

use crate::model::Foreground;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub foreground: Option<Foreground>,
    /// Milliseconds since the last keyboard/mouse input.
    pub idle_ms: u64,
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub fn snapshot() -> Snapshot {
    windows::snapshot()
}

#[cfg(not(windows))]
pub fn snapshot() -> Snapshot {
    // macOS/Linux capture lands in Phase 3.
    Snapshot { foreground: None, idle_ms: 0 }
}

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

#[cfg(target_os = "macos")]
mod macos;

#[cfg(windows)]
pub fn snapshot() -> Snapshot {
    windows::snapshot()
}

#[cfg(target_os = "macos")]
pub fn snapshot() -> Snapshot {
    macos::snapshot()
}

// Linux / other: no native capture yet — the browser extension still works.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn snapshot() -> Snapshot {
    Snapshot { foreground: None, idle_ms: 0 }
}

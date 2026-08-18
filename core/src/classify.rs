//! Auto-first classification for the OS-capture layer.
//!
//! The OS only sees the foreground *application*. Browser activity (domain,
//! YouTube channel) arrives separately from the extension over local IPC and is
//! merged later; here a browser resolves to `Uncategorized` with low confidence
//! so it surfaces in the review queue until the extension enriches it.
//!
//! Priority mirrors the extension: user rules → learned memory → heuristics.
//! (Rules + learned memory are applied in `store` against the DB; this module is
//! the heuristic layer.)

use crate::model::Category;

/// Known applications → category. Substring match on the lowercased process name.
const APP_MAP: &[(&str, Category)] = &[
    // work / dev
    ("code.exe", Category::Work),
    ("devenv.exe", Category::Work),
    ("idea64.exe", Category::Work),
    ("pycharm64.exe", Category::Work),
    ("rider64.exe", Category::Work),
    ("windowsterminal.exe", Category::Work),
    ("powershell.exe", Category::Work),
    ("pwsh.exe", Category::Work),
    ("cmd.exe", Category::Work),
    ("wt.exe", Category::Work),
    // productivity
    ("slack.exe", Category::Productivity),
    ("notion.exe", Category::Productivity),
    ("obsidian.exe", Category::Productivity),
    ("winword.exe", Category::Productivity),
    ("excel.exe", Category::Productivity),
    ("onenote.exe", Category::Productivity),
    ("outlook.exe", Category::Productivity),
    // entertainment
    ("spotify.exe", Category::Entertainment),
    ("vlc.exe", Category::Entertainment),
    ("mpc-hc64.exe", Category::Entertainment),
    // social
    ("discord.exe", Category::Social),
    ("telegram.exe", Category::Social),
    ("whatsapp.exe", Category::Social),
];

const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
];

/// Heuristic classification of a foreground app.
/// Returns `(category, confidence, source)`.
pub fn classify_app(process: &str) -> (Category, f32, &'static str) {
    let p = process.to_lowercase();

    for (needle, cat) in APP_MAP {
        if p.contains(needle) {
            return (*cat, 0.8, "app-heuristic");
        }
    }

    if BROWSERS.iter().any(|b| p == *b) {
        // Real category depends on the site/channel — the extension provides that.
        return (Category::Uncategorized, 0.2, "browser-pending");
    }

    (Category::Uncategorized, 0.2, "no-signal")
}

/// Whether a process is a browser (so the merge layer knows to expect extension data).
pub fn is_browser(process: &str) -> bool {
    BROWSERS.iter().any(|b| *b == process.to_lowercase())
}

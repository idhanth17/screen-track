# Screen Track

Cross-platform (Windows + macOS) screen-time tracker with **automatic**,
context-aware classification. It sorts your time into Work / Productivity / Study /
Entertainment / Social **without you setting a mode** — and it resolves the hard
case (YouTube) at the **channel** level, learning from every correction.

See **[PROJECT_PLAN.md](PROJECT_PLAN.md)** for the full design.

## Status

| Component | State |
|-----------|-------|
| `extension/` — per-site + YouTube-channel classifier (JS MV3) | ✅ tracks all sites, two-way sync with core |
| `rules/` — default classification signals | ✅ |
| `core/` — Rust OS capture + store + classifier + loopback server | ✅ builds & runs |
| `app/dashboard/` — unified dashboard (day nav, week view, inline corrections) | ✅ |
| `app/src-tauri/` — native **tray app** embedding capture + server + dashboard | ✅ builds & runs |

## Run the app

```bash
cargo run -p screen-track-app --manifest-path "D:/Developer/Projects/Screen Track/Cargo.toml"
```

The tray app runs the capture loop + loopback server (`127.0.0.1:47113`) in-process and
shows the dashboard in a native window. Closing the window hides to the tray; the tray
menu has Show / Quit. The browser extension auto-connects to the same server.

## Quick start (the working part)

Load the browser extension and watch YouTube get classified per channel:

→ **[extension/README.md](extension/README.md)**

## Build order

1. Browser extension (done) — the core classification idea, testable today
2. Rust core — foreground app + idle capture → SQLite, with the same auto-first pipeline
3. Tauri dashboard — timeline, category totals, review queue
4. macOS capture port

## Toolchain needed for the Rust core (not yet installed)

- [Rust](https://rustup.rs/) (MSVC toolchain on Windows)
- [Node.js](https://nodejs.org/) (for the Tauri CLI / dev server)
- Tauri prerequisites (WebView2 is preinstalled on Windows 11)

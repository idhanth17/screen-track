# Screen Track

Cross-platform (Windows + macOS) screen-time tracker with **automatic**,
context-aware classification. It sorts your time into Work / Productivity / Study /
Entertainment / Social **without you setting a mode** — and it resolves the hard
case (YouTube) at the **channel** level, learning from every correction.

See **[PROJECT_PLAN.md](PROJECT_PLAN.md)** for the full design.

## Status

| Component | State |
|-----------|-------|
| `extension/` — per-site + YouTube-channel classifier (JS MV3) | ✅ Chrome/Edge/Brave/**Firefox**, no notifications, two-way sync |
| `rules/` — default classification signals | ✅ expanded (100+ domains/apps) |
| `core/` — Rust OS capture + store + classifier + loopback server | ✅ Windows capture; macOS capture written (unverified) |
| `core` **built-in AI** — bundled MiniLM classifier for unknowns | ✅ ships in the app, on by default, offline |
| `app/dashboard/` — unified dashboard (day/week, **review queue**, settings) | ✅ |
| `app/src-tauri/` — native **tray app** with **autostart-on-login** | ✅ builds, bundles to `.msi`/`.exe` |

## Get it running from source

Build the app once, install it, and it starts automatically on every login and lives in
your system tray. The AI model is bundled into the installer — end users never install
anything extra. Below is the full setup for a fresh machine.

### 1. Install the prerequisites (one time)

**Windows**

1. [Rust](https://rustup.rs/) — download & run `rustup-init.exe`, accept defaults (MSVC toolchain).
2. [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) —
   install the **"Desktop development with C++"** workload (gives the MSVC linker Rust needs).
3. [Node.js](https://nodejs.org/) LTS — provides `npx`, used to run the Tauri bundler.
4. [Git](https://git-scm.com/download/win).
5. WebView2 is preinstalled on Windows 10/11. WiX & NSIS are downloaded automatically on first build.

**macOS**

1. Xcode Command Line Tools: `xcode-select --install`
2. [Rust](https://rustup.rs/): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
3. [Node.js](https://nodejs.org/) LTS (or `brew install node`).
4. Git (bundled with the Xcode tools above).

### 2. Clone and fetch the AI model

The ~87 MB model weights are **not** stored in git; a script pulls them into place. Run it once.

**Windows** (PowerShell or Git Bash):

```bash
git clone https://github.com/idhanth17/screen-track.git
cd screen-track
powershell -ExecutionPolicy Bypass -File scripts/fetch-model.ps1
```

**macOS**:

```bash
git clone https://github.com/idhanth17/screen-track.git
cd screen-track
bash scripts/fetch-model.sh
```

### 3. Build the installer

From the repo root:

```bash
cd app
npx @tauri-apps/cli@2 build
```

The bundler produces, under `target/release/bundle/`:

- **Windows** — `msi/Screen Track_<ver>_x64_en-US.msi` and `nsis/Screen Track_<ver>_x64-setup.exe`
- **macOS** — `dmg/Screen Track_<ver>_aarch64.dmg` (and a `.app` under `macos/`)

### 4. Install & run

- **Windows** — double-click the `.msi` (or `.exe`) and follow the installer. Screen Track then
  launches on every login into the system tray.
- **macOS** — open the `.dmg` and drag **Screen Track** to Applications, then launch it once.
  macOS will ask for **Accessibility** permission (System Settings → Privacy & Security →
  Accessibility) so it can see the foreground app — grant it. *(macOS capture is newly ported
  and not yet hardware-verified; see Status.)*

On first run it enables **Start at login** (toggle any time from the tray/menu-bar icon) and
comes up silently in the tray. Closing the window hides to the tray; the menu has
Show dashboard / Start at login / Quit.

### 5. Load the browser extension

The desktop app tracks native apps; the extension adds per-site + per-YouTube-channel detail.
Load it in each browser you use — see **[extension/README.md](extension/README.md)** (Chrome,
Edge, Brave, Firefox). It auto-connects to the running app over `127.0.0.1:47113`.

### Run from source without installing (dev)

```bash
cargo run -p screen-track-app
```

Runs the capture loop + loopback server + AI enrichment in-process and shows the dashboard.
(In a plain `cargo run` the bundled-model path isn't staged, so AI enrichment stays off and
the built-in rules do the classifying; a packaged build has the model.)

## Smart classification (no nagging, no setup)

Built-in rules place most apps, sites and YouTube channels automatically. For the rest, a
small **AI model (MiniLM) is bundled inside the app** and labels unknown channels/sites on
its own, in the background — **nothing to install, no account, no API key, no cost, and
nothing ever leaves your device** (pure-Rust inference via `candle`). Its guesses appear as
normal categories tagged "AI"; correct any once and that channel/site is remembered forever.

Anything the AI is turned off for lands **silently in the Review queue** in the app (there
are no browser notifications) — clear it whenever you like. The built-in AI is on by default;
toggle it in the app's ⚙ Settings.

## Quick start (the working part)

Load the browser extension and watch YouTube get classified per channel:

→ **[extension/README.md](extension/README.md)**

## Build order

1. Browser extension — cross-browser (Chrome/Edge/Brave/Firefox) ✅
2. Rust core — foreground app + idle capture → SQLite, auto-first pipeline ✅ (Windows)
3. Tauri tray app + dashboard — day/week, review queue, autostart, settings ✅
4. Built-in AI enrichment — bundled MiniLM model, on by default ✅
5. Windows `.msi`/`.exe` installer ✅
6. macOS capture port — code written (`core/src/capture/macos.rs`), **needs a Mac to compile & verify**

## Notes

- Prerequisites and step-by-step setup are under **[Get it running from source](#get-it-running-from-source)** above.
- The AI model lives in `app/src-tauri/resources/minilm/` (weights fetched by `scripts/fetch-model.*`,
  everything else committed) and is bundled into the installer — end users need no extra runtime.
- Data is stored locally in `%LOCALAPPDATA%/ScreenTrack/` (Windows) or `~/Library/Application Support/ScreenTrack/` (macOS).

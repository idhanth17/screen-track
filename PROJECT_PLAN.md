# Screen Track — Project Plan & Scope

Cross-platform (Windows + macOS) application to track, categorize, and analyze screen time with **automatic, context-aware classification** and strict privacy controls for company work.

---

## 1. Vision

Help users understand how they spend time on their computer across categories such as **Work**, **Productivity**, **Study**, **Entertainment**, and **Social Media** — **automatically**, without mislabeling ambiguous activity (e.g. YouTube for learning vs leisure) and without capturing sensitive company data.

**Design north star:** The system classifies on its own. The user should be able to install it, forget it's running, and get accurate results — without remembering to flip any switch. Manual input is a *correction* and a *last resort*, never a prerequisite.

---

## 2. Goals

| Goal | Description |
|------|-------------|
| Accurate time tracking | Record login/logout, active vs idle time, and foreground activity per session |
| **Automatic categorization** | Classify activity into meaningful buckets **without requiring user action per session** |
| **Per-channel context** | Resolve YouTube (and similar) at the **channel/video level**, not the app level |
| **Learning system** | Every correction is remembered so the same channel/site is never asked about twice |
| Work privacy | During company work, track duration only — no titles, URLs, or file paths |
| Cross-platform | Single product experience on Windows and macOS |
| Local-first | Data stored locally by default; user owns their data |

---

## 3. Out of Scope (Initial Releases)

- Screen recording, screenshots, or OCR-based classification
- Cloud-first analytics or mandatory account/sync
- Team/employer monitoring or B2B surveillance features
- Mobile device tracking
- Kernel-level or invasive monitoring drivers
- **Blocking or interrupting the user to classify activity** (classification is background + best-effort)

---

## 4. Core Problems & Solutions

### 4.1 Ambiguous content — the classification pipeline (auto-first)

**Problem:** The same app or site can be Study, Entertainment, or Work depending on context. YouTube is the hardest case: **one channel is a university lecture series, the next is comedy sketches** — on the exact same domain.

**Principle change (important):** We do **not** rely on the user setting a mode. Modes are an optional bias, not a requirement. When the system is uncertain, it **makes its best automatic guess, marks it low-confidence, and moves on** — surfacing uncertain items in a *review queue* the user can clear in bulk whenever they like. It never blocks or nags.

**Layered pipeline — first match wins, highest priority first:**

| # | Layer | What it does | Source tag |
|---|-------|--------------|------------|
| 1 | **User rules** | Explicit user overrides, incl. per-channel ("channel X = Study, always") | `rule` |
| 2 | **Learned memory** | The category this exact **channel / domain** resolved to last time (from prior auto-classification the user didn't correct, or from a correction). **This is what makes YouTube work.** | `learned` |
| 3 | **Signal heuristics** | For a *new, unseen* channel/site: classify from content signals (see 4.2) | `heuristic` |
| 4 | **Optional local ML** | On-device refinement from domain + sanitized title + channel signals (later phase) | `ml` |
| 5 | **Focus mode (optional)** | If — and only if — the user has *chosen* to set a mode for a block, it biases/forces the category. Off by default. | `focus_mode` |
| 0 | **Manual correction** | One-click recategorize. **Always writes back to layer 2**, so the channel/site is auto-correct forever after. | `manual` |

**Key idea:** the **channel (or domain) is the atomic unit of memory.** Once a channel has a category, every future segment on it is classified instantly and silently. A brand-new channel gets a best-guess from signals, becomes sticky, and can be corrected once if wrong.

### 4.2 YouTube classification (first-class, in the MVP)

YouTube is not a Phase-2 nicety — it is the headline problem, so it ships in the MVP browser extension.

**Signals the extension captures per video (sanitized, no watch history stored raw):**

- **Channel ID + channel name** — the primary key for learned memory
- **YouTube's own content category** — YouTube tags each video (e.g. *Education, Science & Technology, Howto & Style, News & Politics, Entertainment, Comedy, Gaming, Music*). Read from page metadata.
- **Title keyword signals** — `lecture, tutorial, course, explained, how to, crash course` → study-leaning; `reaction, vlog, funny, gameplay, trailer` → entertainment-leaning
- **Context** — is it from a playlist / subscription the user already flagged?

**Default signal → category mapping for *unseen* channels** (fully overridable):

| YouTube category | Default bucket |
|------------------|----------------|
| Education, Science & Technology, Howto & Style | **Study** |
| News & Politics, Nonprofits & Activism | **Productivity** (news) |
| Entertainment, Comedy, Film & Animation, Trailers | **Entertainment** |
| Gaming | **Entertainment** (unless channel learned as Study/Work) |
| Music | **Entertainment** |
| People & Blogs, Sports, Travel | **Entertainment** |

The moment the user corrects any channel once, layer 2 (learned memory) overrides this mapping for that channel forever.

### 4.3 Company / sensitive work (e.g. Cintix)

**Problem:** Tracking window titles, URLs, or file paths during company work is a privacy violation.

**Solution:** **Privacy Profiles** that record time buckets only — and, critically, that **auto-engage** (no reliance on the user remembering to toggle):

| Captured | Not captured |
|----------|----------------|
| Duration in "Work (private)" category | Window titles |
| Optional generic label ("IDE", "Browser") | URLs and domains |
| Active vs idle | File paths, repo names, project details |

**Auto-engage mechanisms (so the user never has to remember):**

- App blocklist (company IDE/tools → bucket-only tracking, always)
- Domain blocklist (internal sites → no URL stored, always)
- Optional: scheduled work hours / calendar blocks / VPN or SSID detection
- Manual "Privacy Mode" toggle remains, but is a backup, not the primary path

---

## 5. System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Desktop UI (Tauri)                    │
│     Dashboard · Review queue · Rules · Privacy · Modes   │
└─────────────────────────┬───────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────┐
│              Shared Core (Rust)                          │
│  Normalizer → Privacy Filter → Classifier → Storage   │
│                         │                                │
│              Learned-memory store (channel/domain → cat) │
└───────┬─────────────────────────────┬───────────────────┘
        │                             │
┌───────▼────────┐           ┌────────▼──────────────────┐
│ OS Capture     │           │ Browser Extension          │
│ Windows / macOS│           │ domain + sanitized title   │
│ foreground app │           │ + YouTube channel/category │
│ idle detection │           │ (the key ambiguity solver) │
└────────────────┘           └───────────────────────────┘
```

### Recommended stack (chosen)

| Layer | Technology | Why |
|-------|------------|-----|
| Core engine | **Rust** | Clean native foreground/idle APIs on **both** Win + macOS; low CPU |
| Desktop UI | **Tauri** | One lightweight tray+webview across both OSes; tiny footprint |
| Storage | **SQLite** (via `rusqlite`), encrypted at rest | Local-first |
| Browser companion | **TypeScript extension** (Chrome/Edge/Firefox, MV3) | Only reliable source of channel + URL |
| Local IPC | Extension → core over localhost (loopback, token-authed) | Keeps browser data on-device |

---

## 6. Data Model

```
Session
  ├── start, end, device_id
  └── user_id (local)

ActivitySegment
  ├── start, end
  ├── app_name (or "REDACTED")
  ├── domain (nullable)
  ├── channel_id (nullable)         ← YouTube channel primary key
  ├── channel_name (nullable)
  ├── youtube_category (nullable)    ← YouTube's own content category
  ├── title_fingerprint (hash or null in privacy mode)
  ├── category: work | productivity | study | entertainment | social | uncategorized
  ├── confidence: 0.0 – 1.0
  ├── classification_source: rule | learned | heuristic | ml | focus_mode | manual
  ├── needs_review: bool             ← low-confidence → shows in review queue
  └── privacy_level: full | sanitized | bucket_only

CategoryMemory                       ← the learning layer (makes auto-classification stick)
  ├── key_type: channel | domain
  ├── key: <channel_id or domain>
  ├── display_name
  ├── category
  ├── locked: bool                   ← true once user corrects it (never auto-overwritten)
  └── updated_at

Rule                                 ← explicit user overrides (highest priority)
  ├── match_type: app | domain | channel | title_pattern
  ├── pattern
  ├── category
  └── priority
```

---

## 7. Platform Capabilities

| Signal | Windows | macOS |
|--------|---------|-------|
| Login / logout / idle | Yes | Yes |
| Foreground application | Yes | Yes |
| Window title | Partial | Partial (Accessibility permission) |
| Browser URL + YouTube channel | Via extension | Via extension |
| File path / repo | Partial (high privacy risk) | Partial |

**Permissions (macOS):** Accessibility access required for foreground window monitoring. UX must guide the user through granting this.

---

## 8. Feature Scope by Phase

### Phase 0 — Validation (short, overlaps with Phase 1)

- Capture loop prototype: foreground app + idle state every few seconds → SQLite
- Extension prototype: confirm we can read YouTube channel ID + content category reliably
- **Deliverable:** proof the two hardest signals (idle capture + YouTube channel) work

### Phase 1 — MVP (auto-classification that actually works)

**Scope:**

- Foreground app + idle tracking (Windows first)
- Local encrypted SQLite storage
- **Browser extension in the MVP** (domain + sanitized title + **YouTube channel + category**) — moved up because YouTube is the core problem
- **Auto-first classifier**: rules → learned memory → signal heuristics
- **Learned memory** so corrections stick per channel/domain
- Tray app + dashboard: today's timeline, totals by category, **review queue** for low-confidence items
- One-click recategorize with automatic "remember this channel/domain"
- Privacy Mode v1: auto-engage via app/domain blocklist (bucket-only)
- Login-to-logout session view

**Out of scope:** ML, sync, second OS, calendar/VPN auto-privacy, weekly reports

**Success criteria:** Install it, use the machine normally for a day *without touching any control*, and have YouTube split correctly into Study vs Entertainment by channel — with a short review queue, not a stream of interruptions.

### Phase 2 — Classification depth & reporting

- Optional Focus modes (bias layer) for power users
- Priority-ordered rule editor UI
- Weekly summary reports
- Batch review-queue clearing, channel management screen
- Auto-privacy triggers (schedule / calendar / VPN / SSID)

### Phase 3 — Cross-platform parity (macOS)

- Port OS capture layer to macOS
- Unified UI and settings across both
- Platform-specific permission flows (Accessibility)
- Performance and battery optimization

### Phase 4 — Intelligence & polish

- Optional on-device ML for domain/title/channel classification
- Calendar integration for auto focus modes
- Daily/weekly goals and limits
- Idle vs active vs away-from-desk refinement
- Privacy audit screen (show exactly what was recorded)

### Phase 5 — Optional future

- Encrypted multi-device sync (user-owned)
- Custom user-defined categories and tags
- Export / import of rules and data
- API for personal automation

---

## 8.1 MVP Definition (Minimum Shippable Product)

1. Tray/desktop app running in background (Windows)
2. Tracks: timestamp, foreground app, idle state
3. Browser extension feeding domain + **YouTube channel + content category**
4. **Automatic** classification: rules → learned memory → signal heuristics (no mode required)
5. Learned memory: one correction per channel/domain, remembered forever
6. Dashboard: timeline + category breakdown + **review queue** for low-confidence items
7. Auto-engaging privacy redaction via blocklist

---

## 9. Privacy & Trust Requirements

Non-negotiable product requirements:

- All data stored locally by default
- Privacy Mode must prevent capture of titles, URLs, and paths — **and engage automatically** via blocklist so it never depends on memory
- Browser ↔ core traffic stays on loopback; never leaves the device
- User-visible audit: "What did the app record today?"
- No screenshots or screen content analysis
- Clear permission prompts (especially on macOS)
- Graceful degradation on managed/corporate machines where deep monitoring is blocked

---

## 10. Known Limitations

| Limitation | Mitigation |
|------------|------------|
| Same site, different intent (YouTube) | **Per-channel learned memory + YouTube category signal** (primary), rules + one-click correction (backup) |
| New/unseen channel first-guess may be wrong | Best-guess + low-confidence flag + review queue; corrected once, sticky forever |
| Company data exposure | Auto-engaging privacy profiles with bucket-only tracking |
| macOS requires Accessibility permission | Clear onboarding flow |
| App name alone is insufficient for browsers | Browser extension required (in MVP) |
| Employer MDM may restrict monitoring | Privacy mode + local-only; respect corporate policy |
| 100% automatic classification | Very high automatic accuracy is the goal; corrections train the system and shrink over time |

---

## 11. Success Metrics

| Metric | Target |
|--------|--------|
| Daily tracking uptime | > 99% when app is running |
| **Auto-classification accuracy (no user action)** | **> 85% out of the box, > 95% after learning** |
| **User actions required per day** | **Near zero after the first week** (only review-queue clears) |
| Privacy mode leaks | Zero raw titles/URLs stored when engaged |
| Recategorize action | < 5 seconds, and never needed twice for the same channel |
| Idle CPU usage | < 1% average |

---

## 12. First Milestone (Build order)

1. **Rust core** — foreground window polling + idle detection → SQLite (Windows)
2. **SQLite store** — sessions, activity segments, category memory, rules
3. **Classifier** — auto-first pipeline (rules → learned → heuristics) with confidence + needs_review
4. **Browser extension** — YouTube channel + category + domain → core over loopback
5. **Tauri dashboard** — timeline, category totals, review queue, one-click correct (writes learned memory)
6. **Auto privacy** — blocklist-driven redaction at capture time

Dogfood on Windows for ~2 weeks; then port capture to macOS (Phase 3).

---

## 13. Project Structure

```
screen-track/
├── core/                 # Rust: capture, classify, store, privacy, local IPC
│   ├── src/
│   │   ├── capture/      # foreground app + idle (per-OS behind a trait)
│   │   ├── classify/     # auto-first pipeline + learned memory
│   │   ├── store/        # SQLite schema + queries
│   │   ├── privacy/      # blocklist redaction
│   │   └── ipc/          # loopback server for the extension
│   └── platforms/
│       ├── windows/
│       └── macos/
├── app/                  # Tauri desktop shell + dashboard UI
├── extension/            # TypeScript MV3 browser extension (YouTube capture)
├── rules/                # Default classification rules + signal mappings (JSON)
└── PROJECT_PLAN.md       # This document
```

---

## 14. Open Questions

- [ ] Encryption: SQLCipher vs app-level field encryption for v1?
- [ ] Extension → core transport: WebSocket vs HTTP long-poll on loopback?
- [ ] Should news (YouTube "News & Politics") be its own category vs Productivity?
- [ ] Default category set — fixed for MVP, user-extensible in Phase 5?

---

## 15. Summary

**Screen Track is feasible** as a personal productivity tool built around two pillars:

1. **Automatic over asking** — rules, per-channel learned memory, and content signals classify silently; the user only ever *corrects*, and each correction is remembered so it never recurs. YouTube splits into Study vs Entertainment **by channel**, on its own.
2. **Privacy by design** — company work is tracked as time in a bucket, never as content, and privacy mode engages automatically.

Track **time** and **category**, not **content**. Classify automatically; learn from every correction; never nag.

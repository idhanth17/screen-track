# Screen Track — YouTube Classifier (browser extension)

The flagship classification slice, runnable **today with no toolchain**. It tracks
**every site as itself** — `instagram.com` shows as **Instagram**, not "Chrome" —
and special-cases **YouTube to the channel** (plus YouTube's own content category),
auto-sorting your time into **study / entertainment / productivity / social / …**
with corrections remembered forever. Each site/channel is a first-class "entity",
so this data slots straight into the unified multi-source (desktop + mobile) view.

## Load it

The same folder runs on all four browsers (one manifest — Chromium reads
`background.service_worker`, Firefox reads `background.scripts`; every API call
goes through `browser`/`chrome`, both promise-based in MV3).

**Chrome / Edge / Brave** (Chromium)

1. Open `chrome://extensions` (Edge: `edge://extensions`, Brave: `brave://extensions`).
2. Turn on **Developer mode** (top-right).
3. Click **Load unpacked** and select this `extension/` folder.
4. Pin the extension so its icon shows in the toolbar.

**Firefox** (128+)

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on…** and pick any file inside this `extension/`
   folder (e.g. `manifest.json`).
3. The add-on stays until you restart Firefox (that's how temporary add-ons work;
   a signed `.xpi` is only needed for permanent install).

No notifications on any browser — anything the rules can't place lands silently
in the **Review** queue in the Screen Track app, where you reclassify at leisure.

## Try it

1. Play a lecture/tutorial video → open the popup: it lands in **Study**.
2. Play a comedy/gaming video → it lands in **Entertainment**.
3. Disagree with one? Change the dropdown next to the channel. That channel is now
   **locked** to your choice and will never be asked about again — today's totals
   re-tally instantly.
4. Tick **needs review** to see only the low-confidence guesses.

## How it decides (auto-first — no mode to remember)

Priority order, first match wins:

1. **Your rules** (explicit overrides) —
2. **Learned memory** — how this exact channel resolved before (or your correction)
3. **Signal heuristics** — YouTube's content category, then title keywords
4. **Fallback** — `uncategorized`, flagged for the review queue

A new channel gets a best guess and becomes sticky; correcting it once makes it
permanent. See `../PROJECT_PLAN.md` §4.1–4.2.

## What it stores (all on-device, `chrome.storage.local`)

- `memory` — channel/domain → category (your learning layer)
- `daily` — per-day totals + per-channel time
- No history of individual videos, no network calls, nothing leaves the browser.

## Files

| File | Role |
|------|------|
| `content/yt-page.js` | MAIN world: reads YouTube's data (UC channelId + category) |
| `content/yt-dom.js`  | ISOLATED world: nav + play/visibility detection, messaging |
| `background.js`      | time accumulator + classifier + popup API |
| `lib/classifier.js`  | pure auto-first pipeline (portable to the Rust core) |
| `lib/storage.js`     | storage + daily aggregation + corrections |
| `data/rules.js`      | default category signals (mirrors `../rules/default-rules.json`) |
| `popup/`             | today's breakdown + review queue + one-click correct |

## What/how it tracks

- **Any site** → attributed to the site (Instagram, LinkedIn, …) by domain, while
  that tab is the active tab, the browser is focused, and you're not idle.
- **YouTube** → attributed to the **channel** (with content category), via a
  content script that only reads channel/title — no page content is stored.
- Only the **domain** is stored for non-YouTube sites (never path, query, or title).

## Known MVP limits

- Browser-only (native desktop apps are the Rust core's job; mobile is future).
- New/unseen sites & channels get a best guess and a review flag until corrected once.

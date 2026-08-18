"use strict";

const CATS = ["work", "productivity", "study", "entertainment", "social", "uncategorized"];
const CAT_VAR = {
  work: "--work", productivity: "--productivity", study: "--study",
  entertainment: "--entertainment", social: "--social", uncategorized: "--uncategorized"
};

// Transport: Tauri invoke when embedded, else HTTP against the loopback server.
const TAURI = window.__TAURI__ && window.__TAURI__.core;
async function json(url, opts) {
  const r = await fetch(url, Object.assign({ cache: "no-store" }, opts));
  if (!r.ok) throw new Error("HTTP " + r.status);
  return r.json();
}
async function api(cmd, args = {}) {
  if (TAURI) {
    // Tauri commands use snake_case names; map the HTTP-style verbs to them.
    if (cmd === "get_settings") return TAURI.invoke("get_settings");
    if (cmd === "set_settings") return TAURI.invoke("set_settings", { settings: args });
    return TAURI.invoke(cmd, args);
  }
  if (cmd === "overview") return json("/overview" + (args.date ? `?date=${args.date}` : ""));
  if (cmd === "week") return json("/week" + (args.date ? `?date=${args.date}` : ""));
  if (cmd === "correct")
    return json("/correct", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(args) });
  if (cmd === "get_settings") return json("/settings");
  if (cmd === "set_settings")
    return json("/settings", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(args) });
  if (cmd === "extension_status") return json("/status");
  // uninstall/open_url are app-only (Tauri); no HTTP equivalent.
}

// Repo/release URLs for the extension download + docs.
const REPO = "https://github.com/idhanth17/screen-track";
const EXT_ZIP = `${REPO}/releases/latest/download/screen-track-extension.zip`;
const EXT_DOCS = `${REPO}/blob/master/extension/README.md`;

function openExternal(url) {
  if (TAURI) TAURI.invoke("open_url", { url });
  else window.open(url, "_blank");
}

// Short, friendly label for how an item got its category.
function sourceTag(cs) {
  if (cs === "manual") return { txt: "you", cls: "manual" };
  if (cs === "ai") return { txt: "AI", cls: "ai" };
  if (cs === "rule") return { txt: "rule", cls: "" };
  if (cs === "learned") return { txt: "learned", cls: "" };
  return { txt: "auto", cls: "" };
}

// ---- state ----
const state = { date: todayStr(), view: "day", filter: "all" };

function todayStr() {
  const d = new Date(), p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
function addDays(ds, n) {
  const [y, m, d] = ds.split("-").map(Number);
  const dt = new Date(y, m - 1, d + n), p = (x) => String(x).padStart(2, "0");
  return `${dt.getFullYear()}-${p(dt.getMonth() + 1)}-${p(dt.getDate())}`;
}
function prettyDate(ds) {
  const [y, m, d] = ds.split("-").map(Number);
  const dt = new Date(y, m - 1, d);
  if (ds === todayStr()) return "Today";
  if (ds === addDays(todayStr(), -1)) return "Yesterday";
  return dt.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
}

// ---- helpers ----
function fmt(ms) {
  const s = Math.round(ms / 1000), h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${s}s`;
}
function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#64748b";
}
function el(tag, cls, html) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
}
function esc(s) {
  return String(s || "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}
function setStatus(text, cls) {
  const s = document.getElementById("status");
  s.textContent = text;
  s.className = "statusbar" + (cls ? " " + cls : "");
}

// ---- day view ----
function renderDay(ov) {
  document.getElementById("total").textContent = fmt(ov.total_ms) + " tracked";

  const cats = document.getElementById("cats");
  cats.innerHTML = "";
  const max = Math.max(1, ...ov.by_category.map(([, ms]) => ms));
  if (!ov.by_category.length) cats.appendChild(el("div", "empty", "No categories."));
  for (const [name, ms] of ov.by_category) {
    const color = cssVar(CAT_VAR[name] || "--uncategorized");
    const row = el("div", "cat-row");
    row.appendChild(el("span", "cat-name", esc(name)));
    const track = el("span", "cat-track");
    track.appendChild(el("span", "cat-fill")).style.cssText = `width:${(ms / max) * 100}%;background:${color}`;
    row.appendChild(track);
    row.appendChild(el("span", "cat-val", fmt(ms)));
    cats.appendChild(row);
  }

  // The review queue is anything we're unsure about: explicitly flagged, or
  // simply still uncategorized (that's the honest "we don't know" bucket).
  const needsReview = (a) => a.needs_review || a.category === "uncategorized";

  const reviewN = ov.activities.filter(needsReview).length;
  const countEl = document.getElementById("reviewCount");
  countEl.textContent = reviewN;
  countEl.hidden = reviewN === 0;
  document.querySelector(".tab-review").classList.toggle("has-review", reviewN > 0);

  const list = document.getElementById("activities");
  list.innerHTML = "";
  const rows = ov.activities.filter((a) =>
    state.filter === "all" ? true : state.filter === "review" ? needsReview(a) : a.source === state.filter
  );
  const emptyEl = document.getElementById("empty");
  emptyEl.style.display = rows.length ? "none" : "block";
  emptyEl.textContent = state.filter === "review"
    ? "Nothing to review — everything's classified. 🎉"
    : "Nothing tracked for this day.";

  for (const a of rows) {
    const row = el("div", "act");
    row.appendChild(el("span", `badge ${a.source}`, a.source === "youtube" ? "YouTube" : a.source[0].toUpperCase() + a.source.slice(1)));

    const name = el("div", "act-name");
    const title = el("div", "act-title", esc(a.name));
    if (needsReview(a)) title.appendChild(el("span", "review-flag", "needs review"));
    else {
      const tag = sourceTag(a.class_source);
      title.appendChild(el("span", `src-tag ${tag.cls}`, tag.txt));
    }
    name.appendChild(title);
    if (a.detail) name.appendChild(el("div", "act-sub", esc(a.detail)));
    row.appendChild(name);

    const select = el("select", "cat-select");
    for (const c of CATS) {
      const opt = el("option", null, c);
      opt.value = c;
      if (c === a.category) opt.selected = true;
      select.appendChild(opt);
    }
    select.addEventListener("change", async () => {
      try {
        await api("correct", { key: a.key, category: select.value, date: state.date });
        refresh();
      } catch (_) {}
    });
    row.appendChild(select);

    row.appendChild(el("span", "act-time", fmt(a.ms)));
    list.appendChild(row);
  }
}

// ---- week view ----
function renderWeek(days) {
  const wrap = document.getElementById("weekbars");
  wrap.innerHTML = "";
  const maxTotal = Math.max(1, ...days.map((d) => d.total_ms));
  const seen = new Set();

  for (const day of days) {
    const row = el("div", "week-row");
    row.appendChild(el("span", "week-day", prettyDate(day.date)));
    const track = el("span", "week-track");
    for (const [cat, ms] of day.by_category) {
      seen.add(cat);
      const seg = el("span", "week-seg");
      seg.style.cssText = `width:${(ms / maxTotal) * 100}%;background:${cssVar(CAT_VAR[cat] || "--uncategorized")}`;
      seg.title = `${cat}: ${fmt(ms)}`;
      track.appendChild(seg);
    }
    row.appendChild(track);
    row.appendChild(el("span", "week-total", fmt(day.total_ms)));
    wrap.appendChild(row);
  }

  const legend = document.getElementById("weekLegend");
  legend.innerHTML = "";
  for (const cat of CATS.filter((c) => seen.has(c))) {
    const item = el("div", "legend-item");
    item.appendChild(el("span", "dot")).style.background = cssVar(CAT_VAR[cat]);
    item.appendChild(el("span", null, cat));
    legend.appendChild(item);
  }
}

// ---- orchestration ----
async function refresh() {
  document.getElementById("dayLabel").textContent = prettyDate(state.date);
  document.getElementById("next").disabled = state.date >= todayStr();
  document.getElementById("dayView").hidden = state.view !== "day";
  document.getElementById("weekView").hidden = state.view !== "week";

  try {
    if (state.view === "day") {
      renderDay(await api("overview", { date: state.date }));
    } else {
      renderWeek(await api("week", { date: state.date }));
    }
    setStatus((state.date === todayStr() ? "live · " : "") + "updated " + new Date().toLocaleTimeString(), "live");
  } catch (e) {
    setStatus("tracker not reachable — is Screen Track running?", "stale");
  }
}

document.getElementById("prev").addEventListener("click", () => { state.date = addDays(state.date, -1); refresh(); });
document.getElementById("next").addEventListener("click", () => {
  if (state.date < todayStr()) { state.date = addDays(state.date, 1); refresh(); }
});
document.getElementById("todayBtn").addEventListener("click", () => { state.date = todayStr(); refresh(); });
document.getElementById("tabs").addEventListener("click", (e) => {
  const b = e.target.closest(".tab");
  if (!b) return;
  state.filter = b.dataset.src;
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t === b));
  refresh();
});
document.getElementById("viewSeg").addEventListener("click", (e) => {
  const b = e.target.closest(".seg-btn");
  if (!b) return;
  state.view = b.dataset.view;
  document.querySelectorAll(".seg-btn").forEach((t) => t.classList.toggle("active", t === b));
  refresh();
});

refresh();
// Live refresh only when viewing today's day view, and never over an open dropdown.
setInterval(() => {
  if (state.date === todayStr() && state.view === "day") {
    if (document.activeElement && document.activeElement.tagName === "SELECT") return;
    if (!document.getElementById("settingsOverlay").hidden) return; // don't churn under the modal
    refresh();
  }
}, 3000);

// ---- settings modal ----
const overlay = document.getElementById("settingsOverlay");
const aiEnabled = document.getElementById("aiEnabled");
const aiStatusEl = document.getElementById("aiStatus");
const settingsMsg = document.getElementById("settingsMsg");

function syncAiStatus() {
  if (aiEnabled.checked) {
    aiStatusEl.textContent = "Built-in model: active";
    aiStatusEl.className = "ollama-status ok";
  } else {
    aiStatusEl.textContent = "Built-in model: off — unknowns stay in the review queue for you";
    aiStatusEl.className = "ollama-status";
  }
}

async function openSettings() {
  settingsMsg.textContent = "";
  try {
    const s = await api("get_settings");
    aiEnabled.checked = s.aiEnabled !== false; // default on
  } catch (_) { aiEnabled.checked = true; }
  syncAiStatus();
  overlay.hidden = false;
}

async function saveSettings() {
  try {
    await api("set_settings", { aiEnabled: aiEnabled.checked });
    settingsMsg.textContent = "Saved.";
    setTimeout(() => { overlay.hidden = true; }, 500);
  } catch (_) {
    settingsMsg.textContent = "Couldn't save settings.";
  }
}

document.getElementById("settingsBtn").addEventListener("click", openSettings);
document.getElementById("settingsClose").addEventListener("click", () => { overlay.hidden = true; });
overlay.addEventListener("click", (e) => { if (e.target === overlay) overlay.hidden = true; });
document.getElementById("settingsSave").addEventListener("click", saveSettings);
aiEnabled.addEventListener("change", syncAiStatus);

// ---- extension status (live/not-live) ----
const extChip = document.getElementById("extChip");
const extChipText = document.getElementById("extChipText");
const extStatusPill = document.getElementById("extStatusPill");

async function refreshExtStatus() {
  let connected = false;
  try { const st = await api("extension_status"); connected = !!(st && st.connected); } catch (_) {}
  extChip.classList.toggle("chip-on", connected);
  extChip.classList.toggle("chip-off", !connected);
  extChipText.textContent = connected ? "Extension live" : "Extension off";
  if (extStatusPill) {
    extStatusPill.textContent = connected ? "Connected" : "Not connected";
    extStatusPill.className = "pill " + (connected ? "pill-on" : "pill-off");
  }
}
extChip.addEventListener("click", openSettings);
document.getElementById("extDownload").addEventListener("click", () => openExternal(EXT_ZIP));
document.getElementById("extSteps").addEventListener("click", () => openExternal(EXT_DOCS));
refreshExtStatus();
setInterval(refreshExtStatus, 5000);

// ---- uninstall (two-click confirm; native confirm() is unreliable in the webview) ----
const uninstallBtn = document.getElementById("uninstallBtn");
let uninstallArmed = false, uninstallTimer = null;
uninstallBtn.addEventListener("click", async () => {
  if (!TAURI) {
    settingsMsg.textContent = "Open the Screen Track app to uninstall it.";
    return;
  }
  if (!uninstallArmed) {
    uninstallArmed = true;
    uninstallBtn.textContent = "Click again to permanently delete";
    uninstallTimer = setTimeout(() => {
      uninstallArmed = false;
      uninstallBtn.textContent = "Delete app & all data";
    }, 4000);
    return;
  }
  clearTimeout(uninstallTimer);
  uninstallBtn.textContent = "Deleting…";
  try { await api("uninstall_app"); } catch (_) {}
});

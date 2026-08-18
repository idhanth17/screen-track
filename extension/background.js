// Background service worker (module).
//
// One driver — the active tab + browser focus + idle — decides "what web thing
// is in front of the user right now" and accumulates time to it. Every site is
// attributed to ITSELF (instagram.com -> "Instagram"), not to the browser.
// YouTube is special-cased: a content script enriches the active YouTube tab
// with the channel + content category, so it's attributed to the channel.
//
// Time model is timestamp-based so it survives service-worker sleep.

import { classify, memoryKey, friendlyName } from "./lib/classifier.js";
import {
  todayKey, getMemory, getRules, getDaily, getOpenCtx, setOpenCtx,
  addTime, rememberIfNew, reclassifyEntity
} from "./lib/storage.js";

const MAX_INTERVAL_MS = 90_000;

// Bump when the storage shape changes; older data is wiped once on next load.
// v3 = per-entity model (sites + channels) with unified identity keys.
const SCHEMA_VERSION = 3;
let migrated = null;
function ensureMigrated() {
  if (!migrated) {
    migrated = (async () => {
      const { schemaVersion } = await chrome.storage.local.get("schemaVersion");
      if (schemaVersion !== SCHEMA_VERSION) {
        await chrome.storage.local.set({
          daily: {}, memory: {}, notified: {}, openCtx: null, schemaVersion: SCHEMA_VERSION
        });
      }
    })();
  }
  return migrated;
}

// ---- live driver state ----
let browserFocused = true;             // is any Chrome window the OS-foreground?
let idleState = "active";              // chrome.idle state
const ytByTab = {};                    // tabId -> {channelName, channelId, ytCategory, title}

function activeNow() {
  return browserFocused && idleState === "active";
}

function parseDomain(url) {
  try {
    const u = new URL(url);
    if (u.protocol !== "http:" && u.protocol !== "https:") return null; // skip chrome://, newtab, etc.
    return u.hostname.replace(/^www\./, "");
  } catch (_) {
    return null;
  }
}

// Turn the active tab into a classification signal set (or null if untracked).
function signalsForTab(tab) {
  if (!tab || !tab.url) return null;
  const domain = parseDomain(tab.url);
  if (!domain) return null;
  if (domain === "127.0.0.1" || domain === "localhost") return null; // don't track our own dashboard

  if (domain === "youtube.com" || domain.endsWith(".youtube.com")) {
    const yt = ytByTab[tab.id];
    if (yt && yt.channelName) {
      return {
        kind: "youtube", domain: "youtube.com",
        channelName: yt.channelName, channelId: yt.channelId || null,
        ytCategory: yt.ytCategory || null, title: yt.title || null
      };
    }
    // On YouTube but not on a resolved watch page yet — treat as the site.
    return { kind: "site", domain: "youtube.com", appName: "YouTube" };
  }
  return { kind: "site", domain, appName: friendlyName(domain) };
}

async function activeTab() {
  try {
    const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    return tabs && tabs[0];
  } catch (_) {
    return null;
  }
}

// Add elapsed active time to the open entity, then reset the tick clock.
async function flush(now = Date.now()) {
  const ctx = await getOpenCtx();
  if (!ctx || !ctx.active || !ctx.signals || !ctx.decision) return ctx;
  const elapsed = Math.min(now - (ctx.lastTickTs || now), MAX_INTERVAL_MS);
  if (elapsed > 0) await addTime(todayKey(now), ctx.signals, ctx.decision, elapsed);
  ctx.lastTickTs = now;
  await setOpenCtx(ctx);
  return ctx;
}

// The heart: figure out the current entity, bank prior time, switch if needed.
async function recompute() {
  const now = Date.now();
  await flush(now);

  const signals = browserFocused ? signalsForTab(await activeTab()) : null;
  const ctx = await getOpenCtx();
  const newKey = signals ? memoryKey(signals) : null;
  const oldKey = ctx && ctx.signals ? memoryKey(ctx.signals) : null;

  if (newKey === oldKey) {
    // Same entity — just refresh the active flag / tick clock.
    if (ctx) {
      ctx.active = activeNow() && !!signals;
      ctx.lastTickTs = now;
      await setOpenCtx(ctx);
    }
    return;
  }

  if (!signals) {
    await setOpenCtx({ active: false, signals: null, decision: null, lastTickTs: now });
    return;
  }

  const [memory, rules] = await Promise.all([getMemory(), getRules()]);
  const decision = classify(signals, memory, rules);
  await rememberIfNew(decision.memoryKey, decision.category, signals.channelName || signals.appName);
  await setOpenCtx({ active: activeNow(), signals, decision, lastTickTs: now });

  if (decision.category === "uncategorized" && decision.needsReview) {
    await maybeNotify(signals);
  }
}

// Notify once per entity we can't categorize, with quick-fix buttons.
async function maybeNotify(signals) {
  const key = memoryKey(signals);
  if (!key) return;
  const { notified = {} } = await chrome.storage.local.get("notified");
  if (notified[key]) return;
  const name = signals.channelName || signals.appName || signals.domain || key;
  chrome.notifications.create("st:" + key, {
    type: "basic",
    iconUrl: "icons/128.png",
    title: "Screen Track — categorize?",
    message: `Couldn't auto-categorize “${name}”. Pick one, or open the popup for others.`,
    buttons: [{ title: "Study" }, { title: "Entertainment" }],
    priority: 1
  });
  notified[key] = true;
  await chrome.storage.local.set({ notified });
}

// Apply a correction and retag the open context if it's the current entity.
async function applyCorrection(key, category) {
  await reclassifyEntity(todayKey(), key, category);
  const ctx = await getOpenCtx();
  if (ctx && ctx.signals && memoryKey(ctx.signals) === key) {
    ctx.decision = { ...ctx.decision, category, source: "manual", confidence: 1.0, needsReview: false };
    await setOpenCtx(ctx);
  }
}

// Pull corrections made in the dashboard and mirror them into our memory, so
// the extension and dashboard always agree (and our next push won't revert them).
const CORE_BASE = "http://127.0.0.1:47113";
async function pullCorrections() {
  try {
    const res = await fetch(CORE_BASE + "/corrections", { cache: "no-store" });
    if (!res.ok) return;
    const map = await res.json(); // { "channel:..": "study", "domain:..": "social" }
    const { memory = {} } = await chrome.storage.local.get("memory");
    for (const [key, category] of Object.entries(map)) {
      const cur = memory[key];
      if (!cur || cur.category !== category || !cur.locked) {
        await reclassifyEntity(todayKey(), key, category); // locks memory + re-tallies today
      }
    }
  } catch (_) { /* core not running — fine */ }
}

// Push today's browser activity to the local core (SQLite). Silently no-ops
// if the daemon/app isn't running — the extension stays fully functional alone.
const CORE_INGEST_URL = CORE_BASE + "/ingest";
async function pushToCore() {
  await pullCorrections(); // apply dashboard corrections first so we don't overwrite them
  const date = todayKey();
  const daily = await getDaily(date);
  const entities = Object.entries(daily.byEntity || {}).map(([key, e]) => ({
    key,
    kind: e.kind,
    name: e.name,
    domain: e.domain || null,
    channelId: e.channelId || null,
    ytCategory: e.ytCategory || null,
    category: e.category,
    ms: e.ms,
    source: e.source,
    needsReview: !!e.needsReview
  }));
  if (!entities.length) return;
  try {
    await fetch(CORE_INGEST_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ date, entities })
    });
  } catch (_) { /* core not running — fine */ }
}

// ---- serialized message + event handling ----
let chain = Promise.resolve();
function enqueue(fn) {
  const run = chain.then(fn, fn);
  chain = run.catch(() => {});
  return run;
}

// Run the one-time migration FIRST, before any event handler writes data.
enqueue(ensureMigrated);

async function handle(msg, sender) {
  await ensureMigrated();
  switch (msg && msg.type) {
    case "yt-channel": {
      if (sender && sender.tab) ytByTab[sender.tab.id] = msg.signals || {};
      await recompute();
      return { ok: true };
    }
    case "getState": {
      await flush();
      await pushToCore();
      const [daily, memory] = await Promise.all([getDaily(), getMemory()]);
      return { date: todayKey(), daily, memory };
    }
    case "reclassify": {
      await applyCorrection(msg.key, msg.category);
      await pushToCore();
      const [daily, memory] = await Promise.all([getDaily(), getMemory()]);
      return { date: todayKey(), daily, memory };
    }
    default:
      return { ok: false, error: "unknown" };
  }
}

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  enqueue(() => handle(msg, sender)).then(sendResponse, () => sendResponse({ ok: false }));
  return true;
});

chrome.notifications.onButtonClicked.addListener((notifId, idx) => {
  if (!notifId.startsWith("st:")) return;
  const key = notifId.slice(3);
  const category = idx === 0 ? "study" : "entertainment";
  enqueue(async () => {
    await applyCorrection(key, category);
    await pushToCore();
    chrome.notifications.clear(notifId);
  });
});

// ---- events that change "what's in front of the user" ----
chrome.tabs.onActivated.addListener(() => enqueue(recompute));
chrome.tabs.onUpdated.addListener((_id, changeInfo, tab) => {
  if (tab && tab.active && (changeInfo.url || changeInfo.status === "complete")) enqueue(recompute);
});
chrome.tabs.onRemoved.addListener((tabId) => { delete ytByTab[tabId]; });
chrome.windows.onFocusChanged.addListener((windowId) => {
  browserFocused = windowId !== chrome.windows.WINDOW_ID_NONE;
  enqueue(recompute);
});

try {
  chrome.idle.setDetectionInterval(60);
  chrome.idle.onStateChanged.addListener((state) => {
    idleState = state;
    enqueue(recompute);
  });
} catch (_) { /* idle API unavailable */ }

// Heartbeat so continuous viewing is counted even while the SW naps.
chrome.alarms.create("tick", { periodInMinutes: 1 });
chrome.alarms.onAlarm.addListener((a) => {
  if (a.name === "tick") enqueue(async () => { await recompute(); await pushToCore(); });
});
chrome.runtime.onInstalled.addListener(() => chrome.alarms.create("tick", { periodInMinutes: 1 }));

// Initialize focus state, then take a first reading.
enqueue(async () => {
  try {
    const w = await chrome.windows.getLastFocused();
    browserFocused = !!(w && w.focused);
  } catch (_) { /* default true */ }
  await recompute();
  await pushToCore();
});

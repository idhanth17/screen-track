// Thin async wrapper over chrome.storage.local + daily aggregation.
// Schema:
//   memory : { "channel:lofi girl": {category, name, locked, updatedAt}, "domain:instagram.com": {...} }
//   daily  : { "YYYY-MM-DD": { totals:{cat:ms}, byEntity:{ "<key>":{kind,name,domain,channelId,ytCategory,category,ms,confidence,source,needsReview,lastTitle} } } }
//   rules  : [ {match_type, pattern, category, priority} ]      (user overrides; empty for MVP)
//   openCtx: persisted accumulator context (survives service-worker sleep)

import { RULES } from "../data/rules.js";
import { memoryKey } from "./classifier.js";

export function todayKey(ts = Date.now()) {
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function get(keys) {
  return new Promise((res) => chrome.storage.local.get(keys, res));
}
function set(obj) {
  return new Promise((res) => chrome.storage.local.set(obj, res));
}

export async function getMemory() {
  return (await get("memory")).memory || {};
}
export async function getRules() {
  return (await get("rules")).rules || [];
}
export async function getOpenCtx() {
  return (await get("openCtx")).openCtx || null;
}
export async function setOpenCtx(ctx) {
  await set({ openCtx: ctx });
}

function emptyDay() {
  const totals = {};
  for (const c of RULES.categories) totals[c] = 0;
  return { totals, byEntity: {} };
}

export async function getDaily(date = todayKey()) {
  const daily = (await get("daily")).daily || {};
  return daily[date] || emptyDay();
}

export async function getAll() {
  return get(["memory", "daily", "rules"]);
}

/** Persist an auto-classified channel so it becomes "remembered" (sticky, unlocked). */
export async function rememberIfNew(memoryKey, category, name) {
  if (!memoryKey) return;
  const { memory = {} } = await get("memory");
  if (memory[memoryKey]) return; // don't overwrite an existing (esp. locked) entry
  memory[memoryKey] = { category, name: name || "", locked: false, updatedAt: Date.now() };
  await set({ memory });
}

/** Add active time to an entity's (site or YouTube channel) tally for a given day. */
export async function addTime(date, signals, decision, ms) {
  const key = memoryKey(signals);
  if (ms <= 0 || !key) return;
  const store = (await get("daily")).daily || {};
  const day = store[date] || emptyDay();

  day.totals[decision.category] = (day.totals[decision.category] || 0) + ms;

  const name = signals.channelName || signals.appName || signals.domain || key;
  const prev = day.byEntity[key] || { ms: 0 };
  day.byEntity[key] = {
    kind: signals.kind,
    name,
    domain: signals.domain || prev.domain || null,
    channelId: signals.channelId || prev.channelId || null,
    ytCategory: signals.ytCategory || prev.ytCategory || null,
    category: decision.category,
    ms: prev.ms + ms,
    confidence: decision.confidence,
    source: decision.source,
    needsReview: decision.needsReview,
    lastTitle: signals.title || prev.lastTitle || ""
  };

  store[date] = day;
  await set({ daily: store });
}

/**
 * User correction: lock a channel to a category and re-tally today so the
 * dashboard updates immediately. This is the write-back that makes learning stick.
 */
export async function reclassifyEntity(date, key, newCategory) {
  const [{ daily = {} }, { memory = {} }] = await Promise.all([get("daily"), get("memory")]);
  const day = daily[date];
  let name = key;
  if (day && day.byEntity[key]) {
    const e = day.byEntity[key];
    name = e.name || key;
    const ms = e.ms;
    if (e.category !== newCategory) {
      day.totals[e.category] = Math.max(0, (day.totals[e.category] || 0) - ms);
      day.totals[newCategory] = (day.totals[newCategory] || 0) + ms;
    }
    e.category = newCategory;
    e.source = "manual";
    e.confidence = 1.0;
    e.needsReview = false;
    daily[date] = day;
  }
  // Lock the correction even if the entity isn't in today's tally yet
  // (e.g. corrected straight from a notification). `key` is already the full
  // memory key ("channel:.." | "domain:..").
  memory[key] = { category: newCategory, name, locked: true, updatedAt: Date.now() };
  await set({ daily, memory });
}

// Auto-first classification pipeline (mirrors PROJECT_PLAN §4.1).
// First match wins, highest priority first:
//   1. user rules  2. learned memory  3. signal heuristics  4. fallback
// Pure functions only — no storage, no DOM. Easy to unit-test and to port to Rust.

import { RULES } from "../data/rules.js";

const C = RULES.confidence;

/**
 * Stable identity for a YouTube channel. We key by the (normalized) channel
 * NAME rather than the UC id / @handle, because those two are not consistently
 * available across SPA navigations — mixing them splits one channel into two
 * entries and makes corrections fail to stick. The name is always present and
 * is what the user reasons about.
 */
export function channelKey(signals) {
  const name = (signals.channelName || "").toLowerCase().replace(/\s+/g, " ").trim();
  return name || signals.channelId || null;
}

/**
 * Build the identity key for a signal. This is used as BOTH the learned-memory
 * key and the per-entity tally key, so a "YouTube channel" and a "site" are
 * first-class, addressable entities (e.g. "channel:lofi girl", "domain:instagram.com").
 */
export function memoryKey(signals) {
  if (signals.kind === "youtube") {
    const k = channelKey(signals);
    return k ? `channel:${k}` : null;
  }
  if (signals.domain) return `domain:${signals.domain}`;
  return null;
}

/** Human-friendly display name for a site (Instagram, LinkedIn, …). */
export function friendlyName(domain) {
  if (!domain) return "Web";
  if (RULES.siteNames[domain]) return RULES.siteNames[domain];
  const sld = domain.replace(/^www\./, "").split(".")[0];
  return sld ? sld.charAt(0).toUpperCase() + sld.slice(1) : domain;
}

function matchUserRules(signals, rules) {
  if (!Array.isArray(rules) || rules.length === 0) return null;
  const sorted = [...rules].sort((a, b) => (b.priority || 0) - (a.priority || 0));
  const title = (signals.title || "").toLowerCase();
  for (const r of sorted) {
    const pat = (r.pattern || "").toLowerCase();
    if (!pat) continue;
    if (r.match_type === "channel" && signals.channelId === r.pattern) return r.category;
    if (r.match_type === "domain" && signals.domain === r.pattern) return r.category;
    if (r.match_type === "title_pattern" && title.includes(pat)) return r.category;
  }
  return null;
}

function heuristicYouTube(signals) {
  // (a) YouTube's own content category is the strongest first-guess for an unseen channel.
  const cat = signals.ytCategory && RULES.youtubeCategoryMap[signals.ytCategory];
  if (cat) return { category: cat, confidence: C.youtubeCategory, detail: `yt:${signals.ytCategory}` };

  // (b) fall back to weak title keyword signals.
  const title = (signals.title || "").toLowerCase();
  for (const [bucket, words] of Object.entries(RULES.titleSignals)) {
    if (words.some((w) => title.includes(w))) {
      return { category: bucket, confidence: C.titleSignal, detail: "title-keyword" };
    }
  }
  return null;
}

/**
 * Look up a domain in the map, honoring subdomains: "en.wikipedia.org" and
 * "mail.google.com" resolve by walking up to a listed parent ("wikipedia.org",
 * but "mail.google.com" is itself listed so it wins first). First hit, most
 * specific first.
 */
export function domainCategory(domain) {
  if (!domain) return null;
  const labels = domain.split(".");
  for (let i = 0; i < labels.length - 1; i++) {
    const candidate = labels.slice(i).join(".");
    if (RULES.domainMap[candidate]) return RULES.domainMap[candidate];
  }
  return null;
}

function heuristicDomain(signals) {
  const cat = domainCategory(signals.domain);
  if (cat) return { category: cat, confidence: C.domain, detail: "domain-list" };
  return null;
}

/**
 * Classify a signal set against learned memory and user rules.
 * @param {{kind:string, channelId?:string, channelName?:string, ytCategory?:string, title?:string, domain?:string}} signals
 * @param {Object} memory  learned-memory map (key -> {category, locked, ...})
 * @param {Array}  rules   user rules
 * @returns {{category, confidence, source, needsReview, memoryKey, detail}}
 */
export function classify(signals, memory = {}, rules = []) {
  const key = memoryKey(signals);

  // 1. User rules (explicit overrides, highest priority).
  const ruled = matchUserRules(signals, rules);
  if (ruled) {
    return { category: ruled, confidence: C.rule, source: "rule", needsReview: false, memoryKey: key, detail: "user-rule" };
  }

  // 2. Learned memory — the channel/domain we've seen (or corrected) before.
  if (key && memory[key] && memory[key].category) {
    return {
      category: memory[key].category,
      confidence: C.learned,
      source: "learned",
      needsReview: false,
      memoryKey: key,
      detail: memory[key].locked ? "corrected" : "remembered"
    };
  }

  // 3. Signal heuristics (first guess for something unseen).
  const h = signals.kind === "youtube" ? heuristicYouTube(signals) : heuristicDomain(signals);
  if (h) {
    return {
      category: h.category,
      confidence: h.confidence,
      source: "heuristic",
      needsReview: h.confidence < C.reviewThreshold,
      memoryKey: key,
      detail: h.detail
    };
  }

  // 4. Fallback — unknown. Auto-assign uncategorized, flag for the review queue.
  return { category: "uncategorized", confidence: C.fallback, source: "heuristic", needsReview: true, memoryKey: key, detail: "no-signal" };
}

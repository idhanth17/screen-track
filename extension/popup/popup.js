import { RULES } from "../data/rules.js";

const COLORS = {
  work: "#6366f1",
  productivity: "#0ea5e9",
  study: "#22c55e",
  entertainment: "#f59e0b",
  social: "#ec4899",
  uncategorized: "#94a3b8"
};
const CATS = RULES.categories;

const $ = (id) => document.getElementById(id);

function fmt(ms) {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${total}s`;
}

function send(msg) {
  return new Promise((res) => chrome.runtime.sendMessage(msg, res));
}

function render(state) {
  $("date").textContent = state.date || "";
  const daily = state.daily || { totals: {}, byEntity: {} };
  const totals = daily.totals || {};
  const grand = Object.values(totals).reduce((a, b) => a + b, 0);
  const max = Math.max(1, ...Object.values(totals));

  $("total").textContent = fmt(grand) + " tracked";

  // category bars
  const bars = $("bars");
  bars.innerHTML = "";
  CATS.filter((c) => (totals[c] || 0) > 0)
    .sort((a, b) => totals[b] - totals[a])
    .forEach((c) => {
      const row = document.createElement("div");
      row.className = "bar-row";
      row.innerHTML =
        `<span class="bar-label">${c}</span>` +
        `<span class="bar-track"><span class="bar-fill" style="width:${(totals[c] / max) * 100}%;background:${COLORS[c]}"></span></span>` +
        `<span class="bar-val">${fmt(totals[c])}</span>`;
      bars.appendChild(row);
    });

  // entity rows (sites + YouTube channels)
  const reviewOnly = $("reviewOnly").checked;
  const entities = Object.entries(daily.byEntity || {})
    .map(([id, e]) => ({ id, ...e }))
    .filter((e) => (reviewOnly ? e.needsReview : true))
    .sort((a, b) => b.ms - a.ms);

  const list = $("list");
  list.innerHTML = "";
  $("empty").style.display = entities.length ? "none" : "block";

  for (const ent of entities) {
    const row = document.createElement("div");
    row.className = "row";

    const dot = `<span class="dot" style="background:${COLORS[ent.category] || COLORS.uncategorized}"></span>`;
    const subBits = [];
    if (ent.kind === "youtube") {
      subBits.push("YouTube");
      if (ent.ytCategory) subBits.push(ent.ytCategory);
    } else if (ent.domain) {
      subBits.push(ent.domain);
    }
    subBits.push(ent.needsReview ? `<span class="review">needs review</span>` : ent.source);
    const meta =
      `<div class="meta"><div class="name">${escapeHtml(ent.name)}</div>` +
      `<div class="sub">${subBits.join(" · ")}</div></div>`;

    const select = document.createElement("select");
    for (const c of CATS) {
      const opt = document.createElement("option");
      opt.value = c; opt.textContent = c;
      if (c === ent.category) opt.selected = true;
      select.appendChild(opt);
    }
    select.addEventListener("change", async () => {
      const next = await send({ type: "reclassify", key: ent.id, category: select.value });
      if (next) render(next);
    });

    const right = document.createElement("div");
    right.className = "right";
    right.innerHTML = `<span class="time">${fmt(ent.ms)}</span>`;
    right.appendChild(select);

    row.innerHTML = dot + meta;
    row.appendChild(right);
    list.appendChild(row);
  }
}

function escapeHtml(s) {
  return String(s || "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

async function refresh(live = false) {
  // Don't rebuild the list while the user is mid-correction in a dropdown.
  if (live && document.activeElement && document.activeElement.tagName === "SELECT") return;
  const state = await send({ type: "getState" });
  if (state) render(state);
}

$("reviewOnly").addEventListener("change", () => refresh());

// Live updates: getState flushes the accumulator, so totals climb in ~real time.
setInterval(() => refresh(true), 2000);
refresh();

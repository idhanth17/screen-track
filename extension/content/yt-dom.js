// ISOLATED-world content script. Reports the current YouTube video's channel
// to the background, which attributes time to that channel. Timing itself is
// handled centrally by the background tab/focus/idle driver — this script only
// answers "what channel is this tab showing right now?".

let currentVideoId = null;
let pendingPlayerData = null;

// Firefox: promise-based `browser`; Chromium: `chrome`.
const B = globalThis.browser ?? globalThis.chrome;

function send(msg) {
  try {
    // Fire-and-forget; swallow "receiving end does not exist" while the SW naps.
    Promise.resolve(B.runtime.sendMessage(msg)).catch(() => {});
  } catch (_) { /* SW asleep / context invalidated */ }
}

function currentVid() {
  try { return new URL(location.href).searchParams.get("v"); } catch (_) { return null; }
}
function isWatch() {
  return location.pathname === "/watch" && !!currentVid();
}

// --- MAIN-world bridge (reliable UC channelId + content category) ---
window.addEventListener("message", (e) => {
  if (e.source !== window) return;
  const m = e.data;
  if (m && m.source === "screentrack-page" && m.type === "player-data" && pendingPlayerData) {
    pendingPlayerData(m.data);
    pendingPlayerData = null;
  }
});

function requestPlayerData() {
  return new Promise((resolve) => {
    let done = false;
    pendingPlayerData = (d) => { if (!done) { done = true; resolve(d || {}); } };
    window.postMessage({ source: "screentrack", type: "req-player-data" }, "*");
    setTimeout(() => { if (!done) { done = true; pendingPlayerData = null; resolve({}); } }, 700);
  });
}

async function gatherSignals(vid) {
  const pd = await requestPlayerData();
  const fresh = pd.prVideoId && pd.prVideoId === vid;
  let channelId = fresh && pd.prChannelId ? pd.prChannelId : null;
  let channelName = pd.author || null;
  let ytCategory = fresh && pd.prCategory ? pd.prCategory : null;
  let title = pd.title || null;

  const owner = document.querySelector(
    "ytd-video-owner-renderer a.yt-simple-endpoint, #owner a.yt-simple-endpoint, a.ytd-video-owner-renderer"
  );
  if (owner) {
    const href = owner.getAttribute("href") || "";
    if (!channelId) {
      const uc = href.match(/\/channel\/(UC[\w-]+)/);
      if (uc) channelId = uc[1];
      else {
        const handle = href.match(/\/@([\w.-]+)/);
        if (handle) channelId = "@" + handle[1];
      }
    }
    if (!channelName) channelName = (owner.textContent || "").trim() || null;
  }
  if (!channelName) {
    const cn = document.querySelector("#owner #channel-name #text, ytd-channel-name#channel-name #text");
    if (cn) channelName = cn.textContent.trim() || null;
  }
  if (!title) {
    const h1 = document.querySelector(
      "h1.ytd-watch-metadata yt-formatted-string, h1.title yt-formatted-string, h1.title"
    );
    title = (h1 && h1.textContent.trim()) ||
      document.title.replace(/\s*-\s*YouTube\s*$/, "").trim() || null;
  }
  return { kind: "youtube", channelName, channelId, ytCategory, title, domain: "youtube.com", videoId: vid };
}

async function reportVideo(vid) {
  currentVideoId = vid;
  let signals = null;
  for (let i = 0; i < 8; i++) {
    if (currentVid() !== vid) return; // navigated away mid-gather
    signals = await gatherSignals(vid);
    if (signals.channelName) break; // got the key signal
    await new Promise((r) => setTimeout(r, 500));
  }
  if (currentVid() !== vid) return;
  if (signals && signals.channelName) send({ type: "yt-channel", signals });
}

function onNav() {
  const vid = currentVid();
  if (!isWatch()) {
    currentVideoId = null;
    send({ type: "yt-channel", signals: {} }); // clear channel → attributed to "YouTube" site
    return;
  }
  if (vid && vid !== currentVideoId) reportVideo(vid);
}

window.addEventListener("yt-navigate-finish", onNav);
window.addEventListener("popstate", onNav);
setInterval(() => {
  const vid = currentVid();
  if (isWatch() && vid && vid !== currentVideoId) reportVideo(vid);
}, 2000);

onNav();

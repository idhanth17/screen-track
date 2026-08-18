// MAIN-world content script. Runs in the page's JS context so it can read
// YouTube's own data objects (window.ytInitialPlayerResponse and the player).
// It cannot use chrome.* APIs, so it answers requests from the ISOLATED script
// (yt-dom.js) over window.postMessage. Category + UC channelId live here.

(function () {
  function readPlayerData() {
    const out = { gvid: null, author: null, title: null, prVideoId: null, prChannelId: null, prCategory: null };
    try {
      const player = document.querySelector("#movie_player");
      if (player && typeof player.getVideoData === "function") {
        const d = player.getVideoData();
        out.gvid = d && d.video_id ? d.video_id : null;
        out.author = d && d.author ? d.author : null;
        out.title = d && d.title ? d.title : null;
      }
    } catch (_) { /* ignore */ }
    try {
      const pr = window.ytInitialPlayerResponse;
      if (pr) {
        if (pr.videoDetails) {
          out.prVideoId = pr.videoDetails.videoId || null;
          out.prChannelId = pr.videoDetails.channelId || null;
          if (!out.title) out.title = pr.videoDetails.title || null;
        }
        const micro = pr.microformat && pr.microformat.playerMicroformatRenderer;
        if (micro && micro.category) out.prCategory = micro.category;
      }
    } catch (_) { /* ignore */ }
    return out;
  }

  window.addEventListener("message", (e) => {
    if (e.source !== window) return;
    const msg = e.data;
    if (!msg || msg.source !== "screentrack" || msg.type !== "req-player-data") return;
    window.postMessage({ source: "screentrack-page", type: "player-data", data: readPlayerData() }, "*");
  });
})();

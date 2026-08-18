// Default classification data for the extension.
// Mirrors rules/default-rules.json at the repo root (kept in sync manually for now).
// The extension is fully on-device; this is the only "knowledge" it ships with.

export const RULES = {
  version: 1,
  categories: ["work", "productivity", "study", "entertainment", "social", "uncategorized"],

  // YouTube's own content category -> our bucket (first guess for an UNSEEN channel).
  youtubeCategoryMap: {
    "Education": "study",
    "Science & Technology": "study",
    "Howto & Style": "study",
    "News & Politics": "productivity",
    "Nonprofits & Activism": "productivity",
    "Entertainment": "entertainment",
    "Comedy": "entertainment",
    "Film & Animation": "entertainment",
    "Trailers": "entertainment",
    "Gaming": "entertainment",
    "Music": "entertainment",
    "People & Blogs": "entertainment",
    "Sports": "entertainment",
    "Travel & Events": "entertainment",
    "Pets & Animals": "entertainment",
    "Autos & Vehicles": "entertainment"
  },

  // Weak keyword hints from the video title (used only when the category is unknown).
  titleSignals: {
    study: ["lecture", "tutorial", "course", "explained", "how to", "crash course", "lesson",
            "documentary", "masterclass", "walkthrough", "deep dive", "full course", "step by step"],
    entertainment: ["reaction", "vlog", "funny", "gameplay", "trailer", "prank", "unboxing",
                    "highlights", "meme", "tier list", "compilation"]
  },

  // Non-YouTube domains -> bucket (used by the future general-site tracker; included for parity).
  domainMap: {
    "coursera.org": "study",
    "udemy.com": "study",
    "khanacademy.org": "study",
    "edx.org": "study",
    "brilliant.org": "study",
    "stackoverflow.com": "productivity",
    "developer.mozilla.org": "productivity",
    "docs.google.com": "productivity",
    "notion.so": "productivity",
    "github.com": "work",
    "gitlab.com": "work",
    "netflix.com": "entertainment",
    "primevideo.com": "entertainment",
    "hotstar.com": "entertainment",
    "twitch.tv": "entertainment",
    "reddit.com": "social",
    "twitter.com": "social",
    "x.com": "social",
    "instagram.com": "social",
    "facebook.com": "social",
    "linkedin.com": "social"
  },

  // Nice display names for common sites (fallback is the capitalized domain).
  siteNames: {
    "youtube.com": "YouTube",
    "instagram.com": "Instagram",
    "linkedin.com": "LinkedIn",
    "x.com": "X",
    "twitter.com": "Twitter",
    "facebook.com": "Facebook",
    "reddit.com": "Reddit",
    "netflix.com": "Netflix",
    "primevideo.com": "Prime Video",
    "hotstar.com": "Hotstar",
    "twitch.tv": "Twitch",
    "github.com": "GitHub",
    "gitlab.com": "GitLab",
    "stackoverflow.com": "Stack Overflow",
    "developer.mozilla.org": "MDN",
    "docs.google.com": "Google Docs",
    "notion.so": "Notion",
    "coursera.org": "Coursera",
    "udemy.com": "Udemy",
    "khanacademy.org": "Khan Academy",
    "chatgpt.com": "ChatGPT",
    "claude.ai": "Claude",
    "whatsapp.com": "WhatsApp",
    "web.whatsapp.com": "WhatsApp"
  },

  // Confidence assigned per signal, and the threshold below which we flag for review.
  confidence: {
    rule: 1.0,
    learned: 0.98,
    youtubeCategory: 0.8,
    titleSignal: 0.55,
    domain: 0.85,
    fallback: 0.2,
    reviewThreshold: 0.6
  }
};

use serde::{Deserialize, Serialize};

/// The classification buckets. Kept in lockstep with the extension's category list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Work,
    Productivity,
    Study,
    Entertainment,
    Social,
    Uncategorized,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Work => "work",
            Category::Productivity => "productivity",
            Category::Study => "study",
            Category::Entertainment => "entertainment",
            Category::Social => "social",
            Category::Uncategorized => "uncategorized",
        }
    }

    pub fn from_str(s: &str) -> Category {
        match s {
            "work" => Category::Work,
            "productivity" => Category::Productivity,
            "study" => Category::Study,
            "entertainment" => Category::Entertainment,
            "social" => Category::Social,
            _ => Category::Uncategorized,
        }
    }
}

/// The current foreground application (privacy-first: process name only by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foreground {
    /// Executable name, e.g. `"chrome.exe"`.
    pub process: String,
    /// Window title, captured only when privacy rules allow (None by default).
    pub title: Option<String>,
}

/// A resolved slice of activity ready to persist.
#[derive(Debug, Clone)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub app: String,
    pub category: Category,
    pub source: String,
    pub confidence: f32,
    pub idle: bool,
}

impl Segment {
    pub fn duration_ms(&self) -> i64 {
        (self.end_ms - self.start_ms).max(0)
    }
}

/// A per-day browser activity (a site or a YouTube channel) reported by the
/// extension over the loopback ingest endpoint. Field names are camelCase to
/// match the extension's JSON payload directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserEntity {
    pub key: String,   // "domain:instagram.com" | "channel:lofi girl"
    pub kind: String,  // "site" | "youtube"
    pub name: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub yt_category: Option<String>,
    pub category: String,
    pub ms: i64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub needs_review: bool,
}

/// The POST body of the /ingest endpoint.
#[derive(Debug, Deserialize)]
pub struct IngestBody {
    pub date: String, // "YYYY-MM-DD"
    pub entities: Vec<BrowserEntity>,
}

/// The POST body of the /correct endpoint (dashboard correction).
#[derive(Debug, Deserialize)]
pub struct CorrectBody {
    pub key: String,
    pub category: String,
    #[serde(default)]
    pub date: String,
}

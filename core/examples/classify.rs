//! Quick manual check of the bundled MiniLM classifier.
//! Usage: cargo run -p screentrack-core --example classify [model_dir]

use screentrack_core::ai::Classifier;
use screentrack_core::store::AiCandidate;
use std::path::PathBuf;

fn cand(key: &str, kind: &str, name: &str, domain: Option<&str>, yt: Option<&str>) -> AiCandidate {
    AiCandidate {
        key: key.into(),
        kind: kind.into(),
        name: name.into(),
        domain: domain.map(String::from),
        yt_category: yt.map(String::from),
    }
}

fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("app/src-tauri/resources/minilm"));
    println!("loading model from {} ...", dir.display());
    let clf = Classifier::load(&dir)?;

    let samples = vec![
        cand("channel:mit opencourseware", "youtube", "MIT OpenCourseWare", None, Some("Education")),
        cand("channel:pewdiepie", "youtube", "PewDiePie", None, Some("Gaming")),
        cand("channel:veritasium", "youtube", "Veritasium", None, None),
        cand("channel:gordon ramsay", "youtube", "Gordon Ramsay", None, None),
        cand("channel:fireship", "youtube", "Fireship", None, Some("Science & Technology")),
        cand("channel:lofi girl", "youtube", "Lofi Girl", None, Some("Music")),
        cand("domain:github.com", "site", "GitHub", Some("github.com"), None),
        cand("domain:notion.so", "site", "Notion", Some("notion.so"), None),
        cand("domain:instagram.com", "site", "Instagram", Some("instagram.com"), None),
        cand("domain:coursera.org", "site", "Coursera", Some("coursera.org"), None),
        cand("domain:netflix.com", "site", "Netflix", Some("netflix.com"), None),
    ];

    for c in &samples {
        let cat = clf.classify(c);
        println!("  {:<24} -> {:?}", c.name, cat.map(|c| c.as_str()));
    }
    Ok(())
}

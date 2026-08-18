//! Built-in, on-device classification using a bundled **MiniLM** sentence-
//! embedding model (`all-MiniLM-L6-v2`), run in pure Rust via `candle`.
//!
//! There is no daemon, no network, no API key and nothing for the user to
//! install: the ~90 MB model ships inside the app. An unknown channel/site is
//! embedded and matched (cosine similarity) against a short prototype sentence
//! for each category; the closest category wins. The result is cached in learned
//! memory so each entity is only ever embedded once.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::Tokenizer;

use crate::model::Category;
use crate::store::AiCandidate;

/// One short natural-language descriptor per category. The unknown entity's text
/// is compared against these; nearest wins. (Uncategorized is deliberately absent
/// — the model always commits to a real bucket; the user can still correct it.)
const PROTOTYPES: &[(Category, &str)] = &[
    (Category::Work, "professional software development and coding, programming and developer tools, source code, cloud consoles, engineering, company and business software"),
    (Category::Productivity, "documents, spreadsheets, email and calendar, note taking, planning and project management, news and reference reading, writing and AI assistants"),
    (Category::Study, "education and learning, science and technology, online courses and university lectures, tutorials and how-to guides, explainers, knowledge, teaching, research"),
    (Category::Entertainment, "entertainment and leisure, videos and movies and television, music, video games and gaming, sports, comedy and funny clips, cooking, vlogs, streaming"),
    (Category::Social, "social networking and messaging apps such as Instagram, Twitter, Facebook, Reddit and Discord, personal feeds, direct messages and chat, online communities"),
];

pub struct Classifier {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    /// Unit-normalized prototype embeddings, aligned with `PROTOTYPES`.
    protos: Vec<(Category, Vec<f32>)>,
}

impl Classifier {
    /// Load the bundled model from a directory containing `config.json`,
    /// `tokenizer.json` and `model.safetensors`.
    pub fn load(dir: &Path) -> Result<Self> {
        let device = Device::Cpu;

        let cfg_text = std::fs::read_to_string(dir.join("config.json"))
            .with_context(|| format!("reading {}", dir.join("config.json").display()))?;
        let config: Config = serde_json::from_str(&cfg_text).context("parsing bert config")?;

        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("loading tokenizer: {e}"))?;
        // Cap length; channel/site descriptors are short.
        tokenizer.with_truncation(Some(tokenizers::TruncationParams {
            max_length: 64,
            ..Default::default()
        })).map_err(|e| anyhow!("tokenizer truncation: {e}"))?;

        let weights = dir.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, &device)
                .context("loading safetensors")?
        };
        let model = BertModel::load(vb, &config).context("building bert model")?;

        let mut this = Classifier { model, tokenizer, device, protos: Vec::new() };
        // Precompute the category prototype embeddings once.
        let mut protos = Vec::with_capacity(PROTOTYPES.len());
        for (cat, text) in PROTOTYPES {
            protos.push((*cat, this.embed(text)?));
        }
        this.protos = protos;
        Ok(this)
    }

    /// Embed one string into a unit-normalized sentence vector (mean-pooled).
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("tokenize: {e}"))?;
        let ids: Vec<u32> = enc.get_ids().to_vec();
        if ids.is_empty() {
            return Err(anyhow!("empty tokenization"));
        }
        let n = ids.len();
        let token_ids = Tensor::new(ids.as_slice(), &self.device)?.unsqueeze(0)?; // [1, n]
        let token_type_ids = token_ids.zeros_like()?;
        // Single sequence, no padding → attention over all tokens; mask not needed.
        let out = self.model.forward(&token_ids, &token_type_ids, None)?; // [1, n, H]
        let pooled = (out.sum(1)? / (n as f64))?; // mean-pool → [1, H]
        let v: Vec<f32> = pooled.squeeze(0)?.to_vec1::<f32>()?;
        Ok(normalize(v))
    }

    /// Classify one candidate. Always commits to a real category (nearest
    /// prototype); the user can still correct it, which locks the memory.
    pub fn classify(&self, cand: &AiCandidate) -> Option<Category> {
        let text = describe(cand);
        let emb = self.embed(&text).ok()?;
        let mut best: Option<(Category, f32)> = None;
        for (cat, proto) in &self.protos {
            let score = dot(&emb, proto);
            if best.map_or(true, |(_, b)| score > b) {
                best = Some((*cat, score));
            }
        }
        best.map(|(c, _)| c)
    }
}

/// Turn an entity into the sentence we embed.
fn describe(cand: &AiCandidate) -> String {
    if cand.kind == "youtube" {
        let yt = cand
            .yt_category
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|c| format!(", YouTube category {c}"))
            .unwrap_or_default();
        format!("YouTube channel \"{}\"{}", cand.name, yt)
    } else {
        let dom = cand
            .domain
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        format!("Website \"{}\"{}", cand.name, dom)
    }
}

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

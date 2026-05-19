//! Concordance engine.
//!
//! Topic-agnostic term checker. Two Tauri commands:
//!
//! - [`check_text`]      scan text against a user-supplied preset. Two modes:
//!                       `flag-if-found` (hits = terms present in text) and
//!                       `flag-if-missing` (hits = terms absent from text).
//! - [`extract_preset`]  YAKE keyword extraction (Campos et al. 2018, KAIS).
//!                       Single-document, unsupervised, no reference corpus.
//!                       Returns top-N key phrases ranked by YAKE score
//!                       (lower = more important).
//!
//! Concordance is **design-agnostic**: no wordlists ship in the binary. The
//! example presets at `data/presets/` (NSF leaked, AURA Lab extended) are
//! repo artifacts for users to download separately, not built-in defaults.

use regex::Regex;
use serde::{Deserialize, Serialize};
use yake_rust::{get_n_best, Config as YakeConfig, StopWords};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetTerm {
    pub term: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub terms: Vec<PresetTerm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub term: String,
    pub source: Option<String>,
    pub count: usize,
    pub contexts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub mode: String,
    pub hits: Vec<Hit>,
    pub total_terms_checked: usize,
    pub clean: bool,
    pub text_length: usize,
}

const CONTEXT_WINDOW: usize = 40;
const MAX_CONTEXTS_PER_TERM: usize = 3;

fn compile_patterns(terms: &[PresetTerm]) -> Vec<(Regex, &PresetTerm)> {
    let mut out = Vec::with_capacity(terms.len());
    for entry in terms {
        let escaped = regex::escape(&entry.term).replace(' ', r"\s+");
        let pattern = format!(r"(?i)\b{}\b", escaped);
        if let Ok(re) = Regex::new(&pattern) {
            out.push((re, entry));
        }
    }
    out
}

fn snap_to_char_boundary(s: &str, mut idx: usize, forward: bool) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(idx) {
        if forward {
            idx += 1;
            if idx > s.len() {
                return s.len();
            }
        } else {
            if idx == 0 {
                return 0;
            }
            idx -= 1;
        }
    }
    idx
}

fn extract_contexts(text: &str, re: &Regex) -> Vec<String> {
    let mut contexts = Vec::new();
    for m in re.find_iter(text) {
        if contexts.len() >= MAX_CONTEXTS_PER_TERM {
            break;
        }
        let start = snap_to_char_boundary(text, m.start().saturating_sub(CONTEXT_WINDOW), false);
        let end =
            snap_to_char_boundary(text, (m.end() + CONTEXT_WINDOW).min(text.len()), true);
        let mut snippet = text[start..end].replace('\n', " ").trim().to_string();
        if start > 0 {
            snippet = format!("... {}", snippet);
        }
        if end < text.len() {
            snippet.push_str(" ...");
        }
        contexts.push(snippet);
    }
    contexts
}

fn scan(text: &str, patterns: &[(Regex, &PresetTerm)]) -> Vec<Hit> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = patterns
        .iter()
        .filter_map(|(re, entry)| {
            let matches: Vec<_> = re.find_iter(text).collect();
            if matches.is_empty() {
                None
            } else {
                Some(Hit {
                    term: entry.term.clone(),
                    source: entry.source.clone(),
                    count: matches.len(),
                    contexts: extract_contexts(text, re),
                })
            }
        })
        .collect();
    hits.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.term.cmp(&b.term)));
    hits
}

#[tauri::command]
pub fn check_text(
    text: String,
    preset_json: String,
    mode: String,
) -> Result<CheckResult, String> {
    let preset: Preset =
        serde_json::from_str(&preset_json).map_err(|e| format!("preset parse error: {}", e))?;
    let patterns = compile_patterns(&preset.terms);
    let total = patterns.len();
    let text_length = text.len();

    let hits_found = scan(&text, &patterns);

    let (hits, clean) = match mode.as_str() {
        "flag-if-missing" => {
            let found: std::collections::HashSet<&str> =
                hits_found.iter().map(|h| h.term.as_str()).collect();
            let mut missing: Vec<Hit> = preset
                .terms
                .iter()
                .filter(|t| !found.contains(t.term.as_str()))
                .map(|t| Hit {
                    term: t.term.clone(),
                    source: t.source.clone(),
                    count: 0,
                    contexts: Vec::new(),
                })
                .collect();
            missing.sort_by(|a, b| a.term.cmp(&b.term));
            let clean = missing.is_empty();
            (missing, clean)
        }
        _ => {
            let clean = hits_found.is_empty();
            (hits_found, clean)
        }
    };

    Ok(CheckResult {
        mode,
        hits,
        total_terms_checked: total,
        clean,
        text_length,
    })
}

#[tauri::command]
pub fn extract_preset(
    text: String,
    top_n: usize,
    language: String,
    remove_stop_words: bool,
) -> Result<Preset, String> {
    let lang_code = language_to_iso(&language);
    let stops = if remove_stop_words {
        StopWords::predefined(lang_code).ok_or_else(|| {
            format!("YAKE stopwords not available for language '{}'", lang_code)
        })?
    } else {
        StopWords::custom(std::collections::HashSet::new())
    };

    let n = top_n.clamp(1, 500);
    let config = YakeConfig::default();
    let results = get_n_best(n, &text, &stops, &config);

    let terms = results
        .into_iter()
        .map(|r| PresetTerm {
            term: r.keyword,
            source: Some(format!("yake:{:.4}", r.score)),
        })
        .collect::<Vec<_>>();

    let stop_label = if remove_stop_words {
        format!(" stop-words filtered ({}).", lang_code)
    } else {
        " (stop-words retained).".to_string()
    };

    Ok(Preset {
        name: "Extracted Preset".to_string(),
        description: Some(format!(
            "Extracted via YAKE (Campos et al. 2018, KAIS) keyword extraction. \
             Top {} key phrases by YAKE score, lower score = more important.{} \
             n-grams up to {}, Levenshtein dedup at {}.",
            n, stop_label, config.ngrams, config.deduplication_threshold
        )),
        version: Some("0.1.0".to_string()),
        terms,
    })
}

fn language_to_iso(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "es" | "spanish" => "es",
        "fr" | "french" => "fr",
        "de" | "german" => "de",
        "pt" | "portuguese" => "pt",
        "it" | "italian" => "it",
        "nl" | "dutch" => "nl",
        _ => "en",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_preset() -> String {
        serde_json::to_string(&Preset {
            name: "test".to_string(),
            description: None,
            version: None,
            terms: vec![
                PresetTerm { term: "diversity".to_string(), source: Some("user".to_string()) },
                PresetTerm { term: "social justice".to_string(), source: Some("user".to_string()) },
                PresetTerm { term: "absent".to_string(), source: Some("user".to_string()) },
            ],
        })
        .unwrap()
    }

    #[test]
    fn flag_if_found_matches_with_flexible_whitespace() {
        let result = check_text(
            "Our proposal emphasizes diversity and social  justice in outcomes.".to_string(),
            tiny_preset(),
            "flag-if-found".to_string(),
        )
        .unwrap();
        assert_eq!(result.total_terms_checked, 3);
        assert!(!result.clean);
        assert_eq!(result.hits.len(), 2);
        let terms: Vec<&str> = result.hits.iter().map(|h| h.term.as_str()).collect();
        assert!(terms.contains(&"diversity"));
        assert!(terms.contains(&"social justice"));
    }

    #[test]
    fn flag_if_missing_returns_absent_terms() {
        let result = check_text(
            "Our proposal mentions diversity and social justice but nothing else.".to_string(),
            tiny_preset(),
            "flag-if-missing".to_string(),
        )
        .unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].term, "absent");
    }

    #[test]
    fn extract_preset_yake_returns_meaningful_terms() {
        let text = "Machine learning trust calibration is an underexplored area. \
                    Trust calibration helps users understand machine learning outputs. \
                    Researchers have studied trust calibration in adjacent fields.";
        let preset = extract_preset(text.to_string(), 10, "english".to_string(), true).unwrap();
        assert!(!preset.terms.is_empty(), "YAKE returned no terms");
        let terms: Vec<&str> = preset.terms.iter().map(|t| t.term.as_str()).collect();
        let has_trust_calibration = terms.iter().any(|t| t.contains("trust") && t.contains("calibration"));
        assert!(
            has_trust_calibration,
            "Expected a 'trust calibration' phrase in top terms, got: {:?}",
            terms
        );
    }

    #[test]
    fn extract_preset_respects_top_n() {
        let text = "Alpha beta gamma delta epsilon zeta eta theta iota kappa \
                    lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega. \
                    Alpha beta gamma delta epsilon zeta eta theta iota kappa.";
        let preset = extract_preset(text.to_string(), 5, "english".to_string(), true).unwrap();
        assert!(preset.terms.len() <= 5, "got {} terms, expected <= 5", preset.terms.len());
    }

    #[test]
    fn extract_preset_scores_present_and_ordered() {
        let preset = extract_preset(
            "The reviewer praised the proposal. The proposal addressed accessibility. \
             Accessibility is essential for inclusive proposals."
                .to_string(),
            10,
            "english".to_string(),
            true,
        )
        .unwrap();
        assert!(!preset.terms.is_empty());
        let scores: Vec<f64> = preset
            .terms
            .iter()
            .filter_map(|t| {
                t.source
                    .as_ref()
                    .and_then(|s| s.strip_prefix("yake:"))
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .collect();
        assert_eq!(scores.len(), preset.terms.len(), "every term should have a yake score");
        for w in scores.windows(2) {
            assert!(w[0] <= w[1], "YAKE results should be sorted ascending by score");
        }
    }

    #[test]
    fn empty_preset_returns_empty() {
        let empty = serde_json::to_string(&Preset {
            name: "empty".to_string(),
            description: None,
            version: None,
            terms: vec![],
        })
        .unwrap();
        let result = check_text("anything goes here".to_string(), empty, "flag-if-found".to_string()).unwrap();
        assert_eq!(result.total_terms_checked, 0);
        assert!(result.clean);
    }
}

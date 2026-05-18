//! Concordance engine.
//!
//! Topic-agnostic term checker. Two Tauri commands:
//!
//! - [`check_text`]      scan text against a user-supplied preset. Two modes:
//!                       `flag-if-found` (hits = terms present in text) and
//!                       `flag-if-missing` (hits = terms absent from text).
//! - [`extract_preset`]  tokenize input text, optionally drop stop-words,
//!                       return ranked term/frequency list as a new preset.
//!
//! Concordance is **design-agnostic**: no wordlists ship in the binary. The
//! example presets at `data/presets/` (NSF leaked, AURA Lab extended) are
//! repo artifacts for users to download separately, not built-in defaults.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    min_frequency: usize,
    language: String,
    remove_stop_words: bool,
) -> Result<Preset, String> {
    let stops = if remove_stop_words {
        stopword_set(&language)
    } else {
        std::collections::HashSet::new()
    };
    let token_re = Regex::new(r"\b[\p{L}][\p{L}\p{M}'\-]*\b")
        .map_err(|e| format!("token regex: {}", e))?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for m in token_re.find_iter(&text) {
        let raw = m.as_str().to_lowercase();
        if raw.len() < 3 {
            continue;
        }
        if stops.contains(raw.as_str()) {
            continue;
        }
        *counts.entry(raw).or_insert(0) += 1;
    }
    let min = min_frequency.max(1);
    let mut entries: Vec<(String, usize)> =
        counts.into_iter().filter(|(_, c)| *c >= min).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let stop_label = if remove_stop_words {
        format!(" stop-words removed ({})", language)
    } else {
        String::new()
    };
    let terms = entries
        .into_iter()
        .map(|(term, count)| PresetTerm {
            term,
            source: Some(format!("extracted:{}", count)),
        })
        .collect::<Vec<_>>();

    Ok(Preset {
        name: "Extracted Preset".to_string(),
        description: Some(format!(
            "Extracted from input document via tokenization + frequency ranking.{} min_frequency={}.",
            stop_label, min
        )),
        version: Some("0.1.0".to_string()),
        terms,
    })
}

fn stopword_set(language: &str) -> std::collections::HashSet<String> {
    use stop_words::{get, LANGUAGE};
    let lang = match language.to_lowercase().as_str() {
        "es" | "spanish" => LANGUAGE::Spanish,
        "fr" | "french" => LANGUAGE::French,
        "de" | "german" => LANGUAGE::German,
        "pt" | "portuguese" => LANGUAGE::Portuguese,
        "it" | "italian" => LANGUAGE::Italian,
        "nl" | "dutch" => LANGUAGE::Dutch,
        _ => LANGUAGE::English,
    };
    get(lang).into_iter().collect()
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
    fn extract_preset_with_stop_words_removed() {
        let preset = extract_preset(
            "The reviewer praised the proposal. The proposal addressed accessibility.".to_string(),
            1,
            "english".to_string(),
            true,
        )
        .unwrap();
        let terms: Vec<&str> = preset.terms.iter().map(|t| t.term.as_str()).collect();
        assert!(!terms.contains(&"the"));
        assert_eq!(preset.terms[0].term, "proposal");
    }

    #[test]
    fn extract_preset_keeps_stop_words_when_disabled() {
        let preset = extract_preset(
            "The reviewer praised the proposal.".to_string(),
            1,
            "english".to_string(),
            false,
        )
        .unwrap();
        let terms: Vec<&str> = preset.terms.iter().map(|t| t.term.as_str()).collect();
        assert!(terms.contains(&"the"));
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

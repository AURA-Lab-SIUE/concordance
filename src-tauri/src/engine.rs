//! Concordance engine.
//!
//! Ported from the AURA Lab Banned Word Checker `core.py` engine, generalized
//! for the topic-agnostic case (no 3-stage decision tree; just a flat scan with
//! mode toggle and a new "extract preset from document" capability).
//!
//! Three Tauri commands:
//!
//! - [`check_text`]      scan text against a preset's term list; supports
//!                       `flag-if-found` (banned-word-checker style) and
//!                       `flag-if-missing` (CFP-alignment style) modes.
//! - [`extract_preset`]  tokenize input text, drop stop-words, return ranked
//!                       term/frequency list as a new preset.
//! - [`load_bundled_preset`] load one of the presets bundled with the app
//!                           (`nsf-2025-leaked` or `aura-lab-extended`).
//!
//! Engine is plain text in -> structured result out. PDF/DOCX/etc. parsing
//! happens in the frontend (PDF.js + mammoth.js) so the Rust side stays small.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Bundled presets (compiled into the binary; same JSON as ../data/presets/)
// ---------------------------------------------------------------------------

const PRESET_NSF_LEAKED: &str =
    include_str!("../../data/presets/nsf-2025-leaked.json");
const PRESET_AURA_EXTENDED: &str =
    include_str!("../../data/presets/aura-lab-extended.json");

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

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
    /// `"flag-if-found"` or `"flag-if-missing"`.
    pub mode: String,
    /// Terms that triggered the mode (hits when `flag-if-found`,
    /// missing terms when `flag-if-missing`). Sorted by count desc, term asc.
    pub hits: Vec<Hit>,
    /// Total terms in the preset that were scanned.
    pub total_terms_checked: usize,
    /// `true` if nothing triggered the mode (no flagged-if-found hits OR
    /// no flagged-if-missing misses, depending on mode).
    pub clean: bool,
    /// Length of input text in bytes (UTF-8).
    pub text_length: usize,
}

// ---------------------------------------------------------------------------
// Pattern compilation + scanning
// ---------------------------------------------------------------------------

const CONTEXT_WINDOW: usize = 40;
const MAX_CONTEXTS_PER_TERM: usize = 3;

/// Compile preset terms into case-insensitive, word-bounded regexes.
/// Phrase terms (containing spaces) match across whitespace runs (incl. newlines).
fn compile_patterns(terms: &[PresetTerm]) -> Vec<(Regex, &PresetTerm)> {
    let mut out = Vec::with_capacity(terms.len());
    for entry in terms {
        let escaped = regex::escape(&entry.term).replace(' ', r"\s+");
        let pattern = format!(r"(?i)\b{}\b", escaped);
        match Regex::new(&pattern) {
            Ok(re) => out.push((re, entry)),
            Err(_) => {
                // Skip malformed terms silently. In practice should never fire
                // for sane preset data since we escape input.
                continue;
            }
        }
    }
    out
}

/// Walk byte index back/forward to the nearest UTF-8 char boundary.
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
        let start = snap_to_char_boundary(
            text,
            m.start().saturating_sub(CONTEXT_WINDOW),
            false,
        );
        let end = snap_to_char_boundary(
            text,
            (m.end() + CONTEXT_WINDOW).min(text.len()),
            true,
        );
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

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

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
            // "Missing" = preset terms NOT present in text. Useful for CFP-alignment:
            // these are funder-vocabulary terms the proposal isn't using.
            let found: std::collections::HashSet<&str> =
                hits_found.iter().map(|h| h.term.as_str()).collect();
            let missing: Vec<Hit> = preset
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
            let clean = missing.is_empty();
            // Sort missing alphabetically (no count to sort on).
            let mut missing = missing;
            missing.sort_by(|a, b| a.term.cmp(&b.term));
            (missing, clean)
        }
        // Default: "flag-if-found"
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
) -> Result<Preset, String> {
    let stops = stopword_set(&language);
    let token_re = Regex::new(r"\b[\p{L}][\p{L}\p{M}'\-]*\b")
        .map_err(|e| format!("token regex: {}", e))?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for m in token_re.find_iter(&text) {
        let raw = m.as_str().to_lowercase();
        // Skip very short tokens and pure-numeric runs
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
            "Extracted from input document via tokenize + stop-words ({}). \
             Terms ranked by frequency (descending). \
             min_frequency={}.",
            language, min
        )),
        version: Some("0.1.0".to_string()),
        terms,
    })
}

#[tauri::command]
pub fn load_bundled_preset(name: String) -> Result<Preset, String> {
    let raw = match name.as_str() {
        "nsf-2025-leaked" => PRESET_NSF_LEAKED,
        "aura-lab-extended" => PRESET_AURA_EXTENDED,
        other => return Err(format!("unknown bundled preset: {}", other)),
    };
    serde_json::from_str::<Preset>(raw).map_err(|e| format!("bundled preset parse error: {}", e))
}

// ---------------------------------------------------------------------------
// Stop-words
// ---------------------------------------------------------------------------

fn stopword_set(language: &str) -> std::collections::HashSet<String> {
    use stop_words::{get, LANGUAGE};
    let lang = match language.to_lowercase().as_str() {
        "en" | "english" | "" => LANGUAGE::English,
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_preset() -> String {
        serde_json::to_string(&Preset {
            name: "test".to_string(),
            description: None,
            version: None,
            terms: vec![
                PresetTerm {
                    term: "diversity".to_string(),
                    source: Some("leaked".to_string()),
                },
                PresetTerm {
                    term: "social justice".to_string(),
                    source: Some("both".to_string()),
                },
                PresetTerm {
                    term: "absent".to_string(),
                    source: Some("existing".to_string()),
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn flag_if_found_basic() {
        let result = check_text(
            "Our proposal emphasizes diversity and social  justice in outcomes.".to_string(),
            tiny_preset(),
            "flag-if-found".to_string(),
        )
        .unwrap();
        assert_eq!(result.mode, "flag-if-found");
        assert_eq!(result.total_terms_checked, 3);
        assert!(!result.clean);
        // Both "diversity" and "social  justice" (extra whitespace) should hit.
        assert_eq!(result.hits.len(), 2);
        let terms: Vec<&str> = result.hits.iter().map(|h| h.term.as_str()).collect();
        assert!(terms.contains(&"diversity"));
        assert!(terms.contains(&"social justice"));
    }

    #[test]
    fn flag_if_missing_inverts() {
        let result = check_text(
            "Our proposal mentions diversity and social justice but nothing else.".to_string(),
            tiny_preset(),
            "flag-if-missing".to_string(),
        )
        .unwrap();
        assert_eq!(result.mode, "flag-if-missing");
        // "absent" should be the only missing term.
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].term, "absent");
        assert!(!result.clean);
    }

    #[test]
    fn extract_preset_drops_stopwords_and_counts() {
        let preset = extract_preset(
            "The reviewer praised the proposal. The proposal addressed accessibility.".to_string(),
            1,
            "english".to_string(),
        )
        .unwrap();
        let terms: Vec<&str> = preset.terms.iter().map(|t| t.term.as_str()).collect();
        // "the" is a stop-word, should be absent.
        assert!(!terms.contains(&"the"));
        // "proposal" appears twice, should be top.
        assert_eq!(preset.terms[0].term, "proposal");
    }

    #[test]
    fn load_bundled_preset_works() {
        let nsf = load_bundled_preset("nsf-2025-leaked".to_string()).unwrap();
        assert!(!nsf.terms.is_empty());
        let aura = load_bundled_preset("aura-lab-extended".to_string()).unwrap();
        assert!(aura.terms.len() >= nsf.terms.len());
    }

    #[test]
    fn unknown_bundled_preset_errors() {
        assert!(load_bundled_preset("nonexistent".to_string()).is_err());
    }
}

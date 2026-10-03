//! Learning from the user's corrections.
//!
//! After a dictation the user fixes the pasted text by hand, selects the fixed passage and invokes "learn". Handy compares
//! what was heard (raw transcript), what was pasted, and the corrected text, asks the formatter model what is worth
//! remembering, validates the answer, and stores it: new vocabulary words and "wrong -> right" corrections that are applied
//! to future transcripts. The functions in this file are pure (no I/O) so they can be tested; the action that ties them to
//! the clipboard, the history and the model is further down.

use crate::settings::Correction;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

const MAX_ITEMS: usize = 5;
const MAX_TERM_CHARS: usize = 64;
const MAX_PHRASE_WORDS: usize = 4;
pub const MAX_CORRECTED_CHARS: usize = 6000;
pub const MAX_STORED_CORRECTIONS: usize = 200;

pub const LEARN_PROMPT: &str = include_str!("../../fork/prompts/learn_prompt.md");

/// Words that must never become a correction on their own: replacing them everywhere would damage ordinary text.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "to", "too", "two", "of", "in", "on", "at", "for", "is",
    "are", "was", "were", "be", "it", "its", "this", "that", "i", "you", "he", "she", "we", "they",
    "my", "your", "see", "sea", "no", "know", "new", "knew", "one", "won", "there", "their",
    "here", "hear", "write", "right", "by", "buy", "not", "now", "with", "as", "if", "so", "og",
    "i", "å", "er", "en", "et", "det", "den", "de", "som", "på", "av", "for", "til", "med", "har",
    "jeg", "du", "vi", "ikke", "kan", "vil", "skal", "men", "eller", "om", "fra", "ved", "seg",
    "var", "ble", "blir", "så", "da", "når", "hva",
];

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Proposal {
    #[serde(default)]
    pub vocabulary: Vec<String>,
    #[serde(default)]
    pub corrections: Vec<RawCorrection>,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct RawCorrection {
    pub wrong: String,
    pub right: String,
    /// The model's claim that "wrong" is not a real word or phrase in any language, so replacing it blindly is safe.
    /// Anything else is stored as a hint for the formatter (a real word like "fart" can be meant).
    #[serde(default)]
    pub literal: bool,
}

/// What survived validation and may be stored.
#[derive(Debug, Default, PartialEq)]
pub struct Learned {
    pub vocabulary: Vec<String>,
    pub corrections: Vec<Correction>,
}

impl Learned {
    pub fn is_empty(&self) -> bool {
        self.vocabulary.is_empty() && self.corrections.is_empty()
    }

    /// Short text for the overlay, e.g. "DYST, dist -> DYST".
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = self.vocabulary.clone();
        parts.extend(self.corrections.iter().map(|c| {
            format!(
                "{} -> {}{}",
                c.wrong,
                c.right,
                if c.hint { " (hint)" } else { "" }
            )
        }));
        parts.join(", ")
    }
}

/// Keeps user text from closing or opening the tags the prompt uses to fence it.
fn fence(text: &str) -> String {
    text.replace("</", "<\u{200b}/")
        .replace("<raw", "<\u{200b}raw")
        .replace("<pasted", "<\u{200b}pasted")
        .replace("<corrected", "<\u{200b}corrected")
}

pub fn build_prompt(
    raw: &str,
    pasted: &str,
    corrected: &str,
    vocabulary: &[String],
    corrections: &[Correction],
) -> String {
    let known_vocab = if vocabulary.is_empty() {
        "(none)".to_string()
    } else {
        vocabulary.join(", ")
    };
    let known_corr = if corrections.is_empty() {
        "(none)".to_string()
    } else {
        corrections
            .iter()
            .map(|c| format!("{} -> {}", c.wrong, c.right))
            .collect::<Vec<_>>()
            .join("; ")
    };
    // One pass over the template: a value can never introduce (or hijack) another placeholder.
    static PLACEHOLDER: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\{\{(vocabulary|corrections|raw|pasted|corrected)\}\}").unwrap());
    PLACEHOLDER
        .replace_all(LEARN_PROMPT, |caps: &regex::Captures| match &caps[1] {
            "vocabulary" => known_vocab.clone(),
            "corrections" => known_corr.clone(),
            "raw" => fence(raw),
            "pasted" => fence(pasted),
            _ => fence(corrected),
        })
        .into_owned()
}

/// Pulls the JSON object out of a model answer (which may be wrapped in code fences or chatter).
pub fn parse_proposal(answer: &str) -> Option<Proposal> {
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&answer[start..=end]).ok()
}

pub(crate) fn is_clean_term(s: &str) -> bool {
    !s.is_empty()
        && s.chars().count() <= MAX_TERM_CHARS
        && !s
            .chars()
            .any(|c| c.is_control() || c == '[' || c == ']' || c == '<' || c == '>')
}

/// Whole-word, case-insensitive containment (letters and digits count as word characters).
pub fn contains_word_ci(haystack: &str, needle: &str) -> bool {
    word_regex(needle).map_or(false, |re| re.is_match(haystack))
}

fn word_regex(needle: &str) -> Option<Regex> {
    if needle.trim().is_empty() {
        return None;
    }
    // No look-around in this regex engine: the boundary characters are captured instead (groups 1 and 2).
    Regex::new(&format!(
        r"(?i)(^|[^\p{{L}}\p{{N}}]){}($|[^\p{{L}}\p{{N}}])",
        regex::escape(needle.trim())
    ))
    .ok()
}

/// Checks a model proposal against the real texts. The model reads user-controlled text, so nothing it says is trusted:
/// vocabulary must literally occur in the corrected text; a correction's "wrong" must literally occur in the raw transcript
/// and its "right" in the corrected text; stop words, long phrases, duplicates and no-ops are dropped.
pub fn validate(
    p: &Proposal,
    raw: &str,
    corrected: &str,
    known_vocab: &[String],
    known: &[Correction],
) -> Learned {
    let mut out = Learned::default();
    for word in &p.vocabulary {
        let w = word.trim();
        if out.vocabulary.len() >= MAX_ITEMS
            || !is_clean_term(w)
            || w.split_whitespace().count() > MAX_PHRASE_WORDS
        {
            continue;
        }
        let lower = w.to_lowercase();
        let single_stop = w.split_whitespace().count() == 1 && STOPWORDS.contains(&lower.as_str());
        let dup = known_vocab.iter().any(|k| k.eq_ignore_ascii_case(w))
            || out.vocabulary.iter().any(|k| k.eq_ignore_ascii_case(w));
        if !single_stop && !dup && contains_word_ci(corrected, w) {
            out.vocabulary.push(w.to_string());
        }
    }
    for c in &p.corrections {
        let (wrong, right) = (c.wrong.trim(), c.right.trim());
        if out.corrections.len() >= MAX_ITEMS
            || !is_clean_term(wrong)
            || !is_clean_term(right)
            || wrong == right
        {
            continue;
        }
        if wrong.split_whitespace().count() > MAX_PHRASE_WORDS
            || right.split_whitespace().count() > MAX_PHRASE_WORDS
        {
            continue;
        }
        // Unless the model vouches that "wrong" is not a real word, the rule is only a hint to the formatter,
        // which may be an ordinary word; a literal rule may not.
        let hint = !c.literal;
        let single_stop = !hint
            && wrong.split_whitespace().count() == 1
            && STOPWORDS.contains(&wrong.to_lowercase().as_str());
        let dup = known
            .iter()
            .chain(out.corrections.iter())
            .any(|k| k.wrong.eq_ignore_ascii_case(wrong));
        if !single_stop
            && !dup
            && contains_word_ci(raw, wrong)
            && contains_word_ci(corrected, right)
        {
            out.corrections.push(Correction {
                wrong: wrong.to_string(),
                right: right.to_string(),
                hint,
            });
        }
    }
    out
}

/// Applies learned corrections to a transcript: whole words, case-insensitive, literal replacement.
pub fn apply_corrections(text: &str, rules: &[Correction]) -> String {
    let mut out = text.to_string();
    for rule in rules.iter().filter(|r| !r.hint) {
        let Some(re) = word_regex(&rule.wrong) else {
            continue;
        };
        // A boundary character is shared by neighbouring matches ("dist dist"), so repeat until stable.
        for _ in 0..4 {
            let next = re
                .replace_all(&out, |caps: &regex::Captures| {
                    format!("{}{}{}", &caps[1], rule.right, &caps[2])
                })
                .into_owned();
            if next == out {
                break;
            }
            out = next;
        }
    }
    out
}

/// When the selection is much longer than the dictation (a whole paragraph or document), keeps the stretch of words that
/// best overlaps the pasted text.
pub fn best_window(corrected: &str, pasted: &str) -> String {
    let words: Vec<&str> = corrected.split_whitespace().collect();
    let target: Vec<String> = pasted
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    let span = target.len() + 12;
    if words.len() <= span || target.is_empty() {
        return corrected.to_string();
    }
    let set: std::collections::HashSet<&str> = target.iter().map(String::as_str).collect();
    let hits: Vec<bool> = words
        .iter()
        .map(|w| set.contains(w.to_lowercase().as_str()))
        .collect();
    let (mut best, mut best_at) = (0usize, 0usize);
    let mut score: usize = hits[..span].iter().filter(|h| **h).count();
    best = best.max(score);
    for start in 1..=(words.len() - span) {
        score = score + hits[start + span - 1] as usize - hits[start - 1] as usize;
        if score > best {
            best = score;
            best_at = start;
        }
    }
    words[best_at..best_at + span].join(" ")
}

static TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\p{L}\p{N}][\p{L}\p{N}'’-]*").unwrap());

/// Offline fallback: words that appear in the corrected text but not in what was heard or pasted, and that look like names or
/// terms (inner capitals, digits, or a capital letter in the middle of a sentence).
pub fn local_candidates(raw: &str, pasted: &str, corrected: &str) -> Vec<String> {
    let known: std::collections::HashSet<String> = TOKEN
        .find_iter(raw)
        .chain(TOKEN.find_iter(pasted))
        .map(|m| m.as_str().to_lowercase())
        .collect();
    let mut out: Vec<String> = Vec::new();
    for m in TOKEN.find_iter(corrected) {
        let w = m.as_str();
        let before = &corrected[..m.start()];
        let after_terminator =
            before.trim_end().ends_with(['.', '!', '?']) || before.trim().is_empty();
        let sentence_start = after_terminator;
        if known.contains(&w.to_lowercase())
            || w.chars().count() < 3
            || STOPWORDS.contains(&w.to_lowercase().as_str())
        {
            continue;
        }
        let chars: Vec<char> = w.chars().collect();
        let inner_caps = chars.iter().skip(1).any(|c| c.is_uppercase());
        let has_digit = chars.iter().any(|c| c.is_ascii_digit());
        let capitalised_mid = chars[0].is_uppercase() && !sentence_start;
        if (inner_caps || has_digit || capitalised_mid)
            && !out.iter().any(|o| o.eq_ignore_ascii_case(w))
            && out.len() < MAX_ITEMS
        {
            out.push(w.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corr(w: &str, r: &str) -> Correction {
        Correction {
            wrong: w.to_string(),
            right: r.to_string(),
            hint: false,
        }
    }
    fn proposal(json: &str) -> Proposal {
        parse_proposal(json).expect("valid proposal")
    }

    #[test]
    fn parses_answers_with_fences_and_chatter() {
        let p = proposal("Sure!\n```json\n{\"vocabulary\":[\"DYST\"],\"corrections\":[{\"wrong\":\"dist\",\"right\":\"DYST\"}],\"summary\":\"x\"}\n```");
        assert_eq!(p.vocabulary, vec!["DYST"]);
        assert_eq!(p.corrections[0].wrong, "dist");
        assert!(parse_proposal("no json here").is_none());
        assert!(parse_proposal("{broken").is_none());
        assert_eq!(proposal("{}"), Proposal::default());
    }

    #[test]
    fn validation_keeps_real_corrections() {
        let p = proposal(
            r#"{"vocabulary":["DYST"],"corrections":[{"wrong":"dist","right":"DYST","literal":true}]}"#,
        );
        let l = validate(
            &p,
            "we use a system called dist for tracking",
            "we use a system called DYST for tracking",
            &[],
            &[],
        );
        assert_eq!(l.vocabulary, vec!["DYST"]);
        assert_eq!(l.corrections, vec![corr("dist", "DYST")]);
    }

    #[test]
    fn validation_rejects_things_not_in_the_texts() {
        let p = proposal(
            r#"{"vocabulary":["hacked"],"corrections":[{"wrong":"invented","right":"DYST"},{"wrong":"dist","right":"missing"}]}"#,
        );
        let l = validate(&p, "a system called dist", "a system called DYST", &[], &[]);
        assert!(l.is_empty(), "{l:?}");
    }

    #[test]
    fn only_a_literal_claim_makes_an_automatic_rule() {
        let p = proposal(
            r#"{"corrections":[{"wrong":"fart","right":"prompt"},{"wrong":"Superwisper","right":"Superwhisper","literal":true}]}"#,
        );
        let l = validate(
            &p,
            "a fart and Superwisper",
            "a prompt and Superwhisper",
            &[],
            &[],
        );
        assert!(
            l.corrections[0].hint,
            "a real word is only a hint by default"
        );
        assert!(
            !l.corrections[1].hint,
            "a vouched non-word is applied automatically"
        );
    }

    #[test]
    fn hint_rules_may_be_ordinary_words_and_are_not_applied_literally() {
        let p = proposal(
            r#"{"corrections":[{"wrong":"det","right":"de"},{"wrong":"det","right":"de","literal":true}]}"#,
        );
        let l = validate(&p, "ta det med", "ta de med", &[], &[]);
        // the hint (listed first, no "literal") is kept; the literal stop-word rule is a duplicate and refused
        assert_eq!(l.corrections.len(), 1);
        assert!(l.corrections[0].hint);
        let rules = vec![
            Correction {
                wrong: "see".into(),
                right: "sea".into(),
                hint: true,
            },
            corr("dist", "DYST"),
        ];
        assert_eq!(apply_corrections("see dist", &rules), "see DYST");
    }

    #[test]
    fn validation_drops_stopwords_noops_duplicates_and_junk() {
        let p = proposal(
            r#"{"vocabulary":["the","Kari","kari","[x]","a b c d e f"],"corrections":[{"wrong":"see","right":"sea","literal":true},{"wrong":"same","right":"same","literal":true},{"wrong":"carry","right":"Kari","literal":true},{"wrong":"Carry","right":"Kari","literal":true}]}"#,
        );
        let l = validate(
            &p,
            "see the same carry",
            "sea the Kari same Kari a b c d e f [x]",
            &["Known".to_string()],
            &[],
        );
        assert_eq!(l.vocabulary, vec!["Kari"]);
        assert_eq!(l.corrections, vec![corr("carry", "Kari")]);
        let known = validate(
            &p,
            "carry",
            "Kari",
            &["kari".to_string()],
            &[corr("carry", "Kari")],
        );
        assert!(known.is_empty());
    }

    #[test]
    fn validation_caps_the_number_of_items() {
        let words: Vec<String> = (0..9).map(|i| format!("Word{i}")).collect();
        let json = format!(
            "{{\"vocabulary\":{}}}",
            serde_json::to_string(&words).unwrap()
        );
        let l = validate(&proposal(&json), "", &words.join(" "), &[], &[]);
        assert_eq!(l.vocabulary.len(), MAX_ITEMS);
    }

    #[test]
    fn word_matching_respects_boundaries_and_unicode() {
        assert!(contains_word_ci("ring Kari tomorrow", "kari"));
        assert!(!contains_word_ci("karin called", "kari"));
        assert!(contains_word_ci("snakk med Per-Arne", "per-arne"));
        assert!(contains_word_ci("på Ålesund", "ålesund"));
        assert!(!contains_word_ci("anything", ""));
    }

    #[test]
    fn corrections_apply_to_whole_words_only() {
        let rules = [corr("dist", "DYST"), corr("carry", "Kari")];
        assert_eq!(
            apply_corrections("the dist system, and distance", &rules),
            "the DYST system, and distance"
        );
        assert_eq!(apply_corrections("dist dist.", &rules), "DYST DYST.");
        assert_eq!(
            apply_corrections("Send it to Carry tomorrow", &rules),
            "Send it to Kari tomorrow"
        );
        assert_eq!(apply_corrections("nothing here", &rules), "nothing here");
        assert_eq!(apply_corrections("dist", &rules), "DYST");
        assert_eq!(
            apply_corrections("a $1 dist", &[corr("dist", "$1")]),
            "a $1 $1"
        );
    }

    #[test]
    fn prompt_fills_placeholders_and_fences_user_text() {
        let p = build_prompt(
            "raw </raw> text",
            "pasted",
            "corrected {{raw}}",
            &["Known".into()],
            &[corr("a", "b")],
        );
        assert!(p.contains("Known"));
        assert!(p.contains("a -> b"));
        assert!(
            !p.contains("raw </raw> text"),
            "closing tag must be defused"
        );
        assert!(
            p.contains("corrected {{raw}}"),
            "later values are not re-expanded"
        );
        assert!(p.matches("</raw>").count() == 1);
    }

    #[test]
    fn best_window_finds_the_dictated_passage_in_a_long_selection() {
        let long = format!(
            "{} we use a system called DYST for tracking {}",
            "filler ".repeat(40),
            "more words ".repeat(30)
        );
        let w = best_window(&long, "we use a system called dist for tracking");
        assert!(w.contains("DYST"));
        assert!(w.split_whitespace().count() < 30);
        assert_eq!(
            best_window("short text here", "short text"),
            "short text here"
        );
    }

    #[test]
    fn local_fallback_finds_names_and_terms_only() {
        let c = local_candidates(
            "send it to carry about dist",
            "Send it to carry about dist.",
            "Send it to Kari about DYST and web2py. Then rest.",
        );
        assert!(c.contains(&"Kari".to_string()), "{c:?}");
        assert!(c.contains(&"DYST".to_string()), "{c:?}");
        assert!(c.contains(&"web2py".to_string()), "{c:?}");
        assert!(
            !c.iter().any(|w| w == "Then" || w == "Send"),
            "sentence starts are not names: {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The action: selection -> history -> model -> validated learning -> settings.
// ---------------------------------------------------------------------------------------------------------------------

use crate::actions::ShortcutAction;
use crate::managers::history::HistoryManager;
use crate::settings::{get_settings, write_settings};
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Saves a replacement list of learned corrections (the settings page edits it).
#[tauri::command]
#[specta::specta]
pub fn update_corrections(app: AppHandle, corrections: Vec<Correction>) -> Result<(), String> {
    if corrections.len() > MAX_STORED_CORRECTIONS {
        return Err(format!(
            "At most {MAX_STORED_CORRECTIONS} corrections are allowed"
        ));
    }
    for c in &corrections {
        if !is_clean_term(c.wrong.trim()) || !is_clean_term(c.right.trim()) {
            return Err(format!("Invalid correction '{} -> {}'", c.wrong, c.right));
        }
    }
    let mut settings = get_settings(&app);
    settings.corrections = corrections;
    write_settings(&app, settings);
    Ok(())
}

/// Copies the current selection (Ctrl+C), reads it, and puts the user's clipboard back. Returns the text and whether it
/// came from a selection; when nothing was copied the clipboard's own text is used (the user may have copied by hand).
pub(crate) fn capture_selection(app: &AppHandle) -> Option<(String, bool)> {
    let clipboard = app.clipboard();
    let saved_text = clipboard.read_text().ok().filter(|t| !t.is_empty());
    let saved_image = if saved_text.is_none() {
        clipboard.read_image().ok().map(|i| i.to_owned())
    } else {
        None
    };
    let sentinel = format!(
        "\u{1}handy-learn-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    clipboard.write_text(sentinel.clone()).ok()?;

    let sent = crate::clipboard::with_enigo(app, |enigo| crate::input::send_copy_ctrl_c(enigo, 60));
    let mut selection = None;
    if sent.is_ok() {
        for _ in 0..24 {
            std::thread::sleep(Duration::from_millis(25));
            if let Ok(t) = clipboard.read_text() {
                if t != sentinel && !t.is_empty() {
                    selection = Some(t);
                    break;
                }
            }
        }
    } else {
        warn!("Learn: could not send the copy shortcut: {:?}", sent.err());
    }

    match (&saved_text, saved_image) {
        (Some(t), _) => {
            let _ = clipboard.write_text(t.clone());
        }
        (None, Some(image)) => {
            let _ = clipboard.write_image(&image);
        }
        (None, None) => {
            let _ = clipboard.clear();
        }
    }
    match selection {
        Some(t) => Some((t, true)),
        None => saved_text.map(|t| (t, false)),
    }
}

async fn ask_model(settings: &crate::settings::AppSettings, prompt: String) -> Option<Proposal> {
    let provider = settings.active_post_process_provider().cloned()?;
    if provider.id == crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID {
        return None;
    }
    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    if model.trim().is_empty() {
        return None;
    }
    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    let disable_reasoning = matches!(provider.id.as_str(), "custom" | "openrouter");
    match crate::llm_client::send_chat_completion(
        &provider,
        api_key,
        &model,
        prompt,
        disable_reasoning,
    )
    .await
    {
        Ok(Some(answer)) => parse_proposal(&answer),
        Ok(None) => None,
        Err(err) => {
            warn!("Learn: the model request failed: {}", err);
            None
        }
    }
}

/// Shows a short result in the overlay once the "processing" overlay has finished fading.
pub(crate) fn announce(app: &AppHandle, kind: &'static str, value: String) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        crate::overlay::show_notice_overlay(&handle, kind, &value);
    });
}

async fn learn_from(app: &AppHandle, captured: Option<(String, bool)>) {
    let Some((selected, _from_selection)) = captured else {
        announce(app, "learned-none", String::new());
        return;
    };
    let history = app.state::<Arc<HistoryManager>>();
    let entry = match history.get_latest_completed_entry() {
        Ok(Some(entry)) => entry,
        _ => {
            warn!("Learn: there is no dictation in the history yet");
            announce(app, "learned-none", String::new());
            return;
        }
    };
    let raw = entry.transcription_text;
    let pasted = entry
        .post_processed_text
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| raw.clone());
    let bounded: String = selected.chars().take(MAX_CORRECTED_CHARS).collect();
    let corrected = best_window(&bounded, &pasted);
    if corrected.trim() == pasted.trim() || corrected.trim() == raw.trim() {
        announce(app, "learned-none", String::new());
        return;
    }

    crate::utils::show_processing_overlay(app);
    let settings = get_settings(app);
    let prompt = build_prompt(
        &raw,
        &pasted,
        &corrected,
        &settings.custom_words,
        &settings.corrections,
    );
    let proposal = ask_model(&settings, prompt).await;
    let used_model = proposal.is_some();
    let learned = match &proposal {
        Some(p) => validate(
            p,
            &raw,
            &corrected,
            &settings.custom_words,
            &settings.corrections,
        ),
        None => Learned {
            vocabulary: local_candidates(&raw, &pasted, &corrected)
                .into_iter()
                .filter(|w| {
                    !settings
                        .custom_words
                        .iter()
                        .any(|k| k.eq_ignore_ascii_case(w))
                })
                .collect(),
            corrections: Vec::new(),
        },
    };
    crate::utils::hide_recording_overlay(app);

    if learned.is_empty() {
        announce(app, "learned-none", String::new());
        return;
    }
    let mut settings = get_settings(app);
    for word in &learned.vocabulary {
        if !settings
            .custom_words
            .iter()
            .any(|k| k.eq_ignore_ascii_case(word))
        {
            settings.custom_words.push(word.clone());
        }
    }
    for c in &learned.corrections {
        if settings.corrections.len() < MAX_STORED_CORRECTIONS
            && !settings
                .corrections
                .iter()
                .any(|k| k.wrong.eq_ignore_ascii_case(&c.wrong))
        {
            settings.corrections.push(c.clone());
        }
    }
    write_settings(app, settings);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "setting": "corrections", "value": learned.describe() }),
    );
    info!(
        "Learn ({}): {}",
        if used_model {
            "model"
        } else {
            "local fallback"
        },
        learned.describe()
    );
    announce(app, "learned", learned.describe());
}

/// "Learn from correction": select the text you fixed, then press the shortcut (or run `handy --learn`).
pub(crate) struct LearnAction;

impl LearnAction {
    fn run(app: &AppHandle) {
        let app = app.clone();
        std::thread::spawn(move || {
            // The copy shortcut must be sent from the main thread, like the paste shortcut.
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = tx.send(capture_selection(&handle));
            });
            let captured = rx.recv_timeout(Duration::from_secs(5)).ok().flatten();
            tauri::async_runtime::block_on(learn_from(&app, captured));
        });
    }
}

impl ShortcutAction for LearnAction {
    // Hotkey: act on release, when the user's modifier keys are up (a held Alt would turn Ctrl+C into Ctrl+Alt+C).
    // CLI: there is no key release, so act immediately.
    fn start(&self, app: &AppHandle, _binding_id: &str, shortcut_str: &str) {
        if shortcut_str == "CLI" {
            Self::run(app);
        }
    }

    fn stop(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        Self::run(app);
    }
}

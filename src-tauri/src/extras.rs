//! Dictation extras that are specific to this fork: prompt variables, text snippets and
//! re-running the last dictation with the next prompt. Kept in one module so upstream
//! merges only have to deal with the few hooks that call into it.

use crate::actions::{
    announce_setting_change, next_in_cycle, process_transcription_output, ShortcutAction,
};
use crate::managers::history::HistoryManager;
use crate::settings::{get_settings, AppSettings, Snippet};
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Longest clipboard text passed into a prompt, and snippet limits.
const MAX_CLIPBOARD_CHARS: usize = 6000;
const MAX_SNIPPETS: usize = 100;
const MAX_SNIPPET_NAME_CHARS: usize = 50;
const MAX_SNIPPET_TEXT_CHARS: usize = 5000;

static VARIABLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{(vocabulary|snippets|clipboard|examples)\}").unwrap());
static SNIPPET_TAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[\s*snippet\s*:([^\]\n]*)\]\]").unwrap());

fn join_or(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        empty.to_string()
    } else {
        items.join(", ")
    }
}

/// Clipboard text as it goes into a prompt: bounded, and with the transcript placeholder
/// defused so the later `${output}` substitution cannot inject text into it.
fn clipboard_for_prompt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "(the clipboard is empty)".to_string();
    }
    let bounded: String = trimmed.chars().take(MAX_CLIPBOARD_CHARS).collect();
    bounded.replace("${output}", "$ {output}")
}

/// Replaces `${vocabulary}`, `${snippets}` and `${clipboard}` in a prompt template.
/// Substituted text is never scanned again, so values cannot introduce new variables.
pub(crate) fn expand_variables(
    template: &str,
    vocabulary: &[String],
    snippet_names: &[String],
    clipboard: &str,
    examples: &str,
) -> String {
    VARIABLE
        .replace_all(template, |caps: &regex::Captures| match &caps[1] {
            "vocabulary" => join_or(vocabulary, "(none yet)"),
            "snippets" => join_or(snippet_names, "(none)"),
            "examples" => examples_block(examples),
            _ => clipboard_for_prompt(clipboard),
        })
        .into_owned()
}

/// The examples as the model sees them. Defuses the transcript placeholder like the clipboard.
fn examples_block(examples: &str) -> String {
    let trimmed = examples.trim();
    if trimmed.is_empty() {
        return "(no examples)".to_string();
    }
    format!(
        "<examples>\n{}\n</examples>",
        trimmed.replace("${output}", "$ {output}")
    )
}

/// Returns settings whose selected prompt has its variables filled in. The clipboard is
/// only read when the prompt actually uses `${clipboard}`.
pub(crate) fn expand_prompt_variables(app: &AppHandle, mut settings: AppSettings) -> AppSettings {
    let Some(selected) = settings.post_process_selected_prompt_id.clone() else {
        return settings;
    };
    let Some(index) = settings
        .post_process_prompts
        .iter()
        .position(|p| p.id == selected)
    else {
        return settings;
    };
    let template = settings.post_process_prompts[index].prompt.clone();
    let examples = settings.post_process_prompts[index].examples.clone();
    let places_examples = template.contains("${examples}");
    let has_examples = !examples.trim().is_empty();
    if !VARIABLE.is_match(&template) && !has_examples {
        return settings;
    }
    let clipboard = if template.contains("${clipboard}") {
        app.clipboard().read_text().unwrap_or_default()
    } else {
        String::new()
    };
    let names: Vec<String> = settings.snippets.iter().map(|s| s.name.clone()).collect();
    let mut expanded = expand_variables(
        &template,
        &settings.custom_words,
        &names,
        &clipboard,
        &examples,
    );
    if has_examples && !places_examples {
        expanded.push_str(
            "\n\nExamples of the expected result (a dictation, then what the output should be). \
             Follow their structure and style:\n",
        );
        expanded.push_str(&examples_block(&examples));
    }
    settings.post_process_prompts[index].prompt = expanded;
    settings
}

/// Replaces `[[snippet: NAME]]` tags (emitted by the post-processing prompt) with the stored
/// snippet text. The text is inserted verbatim; unknown names are dropped.
pub(crate) fn expand_snippets(text: &str, snippets: &[Snippet]) -> String {
    SNIPPET_TAG
        .replace_all(text, |caps: &regex::Captures| {
            let name = caps[1].trim();
            match snippets.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
                Some(snippet) => snippet.text.clone(),
                None => {
                    warn!("Snippet tag for unknown snippet '{}' dropped", name);
                    String::new()
                }
            }
        })
        .into_owned()
}

fn is_valid_snippet_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.chars().count() <= MAX_SNIPPET_NAME_CHARS
        && !name.chars().any(|c| c.is_control() || c == '[' || c == ']')
}

fn validate_snippets(snippets: &[Snippet]) -> Result<(), String> {
    if snippets.len() > MAX_SNIPPETS {
        return Err(format!("At most {MAX_SNIPPETS} snippets are allowed"));
    }
    for (i, snippet) in snippets.iter().enumerate() {
        if !is_valid_snippet_name(&snippet.name) {
            return Err(format!("Invalid snippet name '{}'", snippet.name));
        }
        if snippet.text.chars().count() > MAX_SNIPPET_TEXT_CHARS {
            return Err(format!("Snippet '{}' is too long", snippet.name));
        }
        if snippets[..i]
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&snippet.name))
        {
            return Err(format!("Duplicate snippet name '{}'", snippet.name));
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_snippets(app: AppHandle, snippets: Vec<Snippet>) -> Result<(), String> {
    validate_snippets(&snippets)?;
    let mut settings = get_settings(&app);
    settings.snippets = snippets;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// True when post-processing is fully set up (provider, model, prompt) and there is text, i.e.
/// when a missing result means the request failed and not that it was never meant to run.
pub(crate) fn post_processing_configured(settings: &AppSettings, text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let Some(provider) = settings.active_post_process_provider() else {
        return false;
    };
    let model_set = settings
        .post_process_models
        .get(&provider.id)
        .is_some_and(|m| !m.trim().is_empty());
    let prompt_set = settings
        .post_process_selected_prompt_id
        .as_ref()
        .and_then(|id| settings.post_process_prompts.iter().find(|p| &p.id == id))
        .is_some_and(|p| !p.prompt.trim().is_empty());
    model_set && prompt_set
}

/// True for dictations short enough to skip the language model (see
/// `post_process_min_words`).
pub(crate) fn is_short_utterance(settings: &AppSettings, text: &str) -> bool {
    let min = settings.post_process_min_words as usize;
    min > 0 && text.split_whitespace().count() < min
}

/// Spoken punctuation words (English and Norwegian) and the text they stand for.
const SPOKEN_PUNCTUATION: &[(&str, &str)] = &[
    ("question mark", "?"),
    ("spørsmålstegn", "?"),
    ("exclamation mark", "!"),
    ("exclamation point", "!"),
    ("utropstegn", "!"),
    ("semicolon", ";"),
    ("semikolon", ";"),
    ("colon", ":"),
    ("kolon", ":"),
    ("comma", ","),
    ("komma", ","),
    ("full stop", "."),
    ("period", "."),
    ("punktum", "."),
    ("new paragraph", "\n\n"),
    ("nytt avsnitt", "\n\n"),
    ("new line", "\n"),
    ("ny linje", "\n"),
];

static SPOKEN_PUNCTUATION_RES: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    SPOKEN_PUNCTUATION
        .iter()
        .map(|(word, symbol)| {
            // Eat the spaces around the word and any punctuation the recognizer added itself.
            let pattern = format!(r"(?i)[ \t]*\b{}\b[ \t]*[,.;:!?]?", regex::escape(word));
            (Regex::new(&pattern).unwrap(), *symbol)
        })
        .collect()
});
static SENTENCE_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^|[.!?]\s+|\n+)(\p{Ll})").unwrap());
static EXTRA_SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]{2,}").unwrap());

/// Local, model-free cleanup used when the language model is skipped or unreachable: turns
/// spoken punctuation into symbols and capitalizes sentence starts. Deliberately simple; a
/// literal phrase like "the period of time" is converted too.
pub(crate) fn local_cleanup(text: &str) -> String {
    let mut out = text.trim().to_string();
    for (re, symbol) in SPOKEN_PUNCTUATION_RES.iter() {
        out = re.replace_all(&out, *symbol).into_owned();
    }
    out = EXTRA_SPACES.replace_all(&out, " ").into_owned();
    // Capitalize the first letter of the text and of each sentence or line.
    out = SENTENCE_START
        .replace_all(&out, |caps: &regex::Captures| {
            format!("{}{}", &caps[1], caps[2].to_uppercase())
        })
        .into_owned();
    out.trim().to_string()
}

/// Tells the user that only basic local cleanup was applied. Shown after the overlay has
/// finished with the current dictation.
pub(crate) fn notify_fallback(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(700));
        crate::overlay::show_notice_overlay(&handle, "fallback", "");
    });
}

/// Pastes the most recent dictation again (the final text, after any post-processing).
pub(crate) struct PasteLastAction;

impl ShortcutAction for PasteLastAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let history = app.state::<Arc<HistoryManager>>();
        let text = match history.get_latest_completed_entry() {
            Ok(Some(entry)) => entry
                .post_processed_text
                .filter(|t| !t.trim().is_empty())
                .unwrap_or(entry.transcription_text),
            Ok(None) => {
                warn!("Paste last: there is no dictation in the history yet");
                return;
            }
            Err(err) => {
                warn!("Paste last: could not read the history: {}", err);
                return;
            }
        };
        if text.trim().is_empty() {
            return;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Err(err) = crate::utils::paste(text, handle.clone()) {
                warn!("Paste last: failed to paste: {}", err);
            }
        });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

/// Re-runs the most recent dictation through the next post-processing prompt and pastes the
/// result. Select the earlier pasted text first to have it replaced.
pub(crate) struct RerunAction;

impl ShortcutAction for RerunAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { rerun_with_next_prompt(&app).await });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

async fn rerun_with_next_prompt(app: &AppHandle) {
    let history = app.state::<Arc<HistoryManager>>();
    let raw = match history.get_latest_completed_entry() {
        Ok(Some(entry)) => entry.transcription_text,
        Ok(None) => {
            warn!("Re-run: there is no dictation in the history yet");
            return;
        }
        Err(err) => {
            warn!("Re-run: could not read the history: {}", err);
            return;
        }
    };
    if raw.trim().is_empty() {
        return;
    }

    let mut settings = get_settings(app);
    let ids: Vec<String> = settings
        .post_process_prompts
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let Some(next_id) =
        next_in_cycle(&ids, settings.post_process_selected_prompt_id.as_deref()).cloned()
    else {
        return;
    };
    let name = settings
        .post_process_prompts
        .iter()
        .find(|p| p.id == next_id)
        .map_or_else(|| next_id.clone(), |p| p.name.clone());
    settings.post_process_selected_prompt_id = Some(next_id.clone());
    announce_setting_change(
        app,
        settings,
        "post_process_selected_prompt_id",
        &next_id,
        "prompt",
        &name,
    );

    crate::utils::show_processing_overlay(app);
    let processed = process_transcription_output(app, &raw, true).await;
    let handle = app.clone();
    let text = processed.final_text;
    let _ = app.run_on_main_thread(move || {
        if !text.is_empty() {
            if let Err(err) = crate::utils::paste(text, handle.clone()) {
                warn!("Re-run: failed to paste: {}", err);
            }
        }
        crate::utils::hide_recording_overlay(&handle);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn snippet(name: &str, text: &str) -> Snippet {
        Snippet {
            name: name.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn variables_are_expanded() {
        let out = expand_variables(
            "V: ${vocabulary}\nS: ${snippets}\nC: ${clipboard}",
            &names(&["DYST", "Kubernetes"]),
            &names(&["calendar"]),
            "  hello  ",
            "",
        );
        assert_eq!(out, "V: DYST, Kubernetes\nS: calendar\nC: hello");
    }

    #[test]
    fn empty_values_get_placeholders() {
        let out = expand_variables(
            "${vocabulary}|${snippets}|${clipboard}|${examples}",
            &[],
            &[],
            "  ",
            "",
        );
        assert_eq!(
            out,
            "(none yet)|(none)|(the clipboard is empty)|(no examples)"
        );
    }

    #[test]
    fn substituted_text_is_not_expanded_again() {
        let out = expand_variables("${clipboard}", &[], &[], "${vocabulary} and ${output}", "");
        assert_eq!(out, "${vocabulary} and $ {output}");
    }

    #[test]
    fn examples_are_wrapped_and_defused() {
        let out = expand_variables("A ${examples} B", &[], &[], "", "in -> ${output}");
        assert_eq!(out, "A <examples>\nin -> $ {output}\n</examples> B");
    }

    #[test]
    fn local_cleanup_handles_spoken_punctuation() {
        assert_eq!(
            local_cleanup("are you coming question mark i hope so exclamation mark"),
            "Are you coming? I hope so!"
        );
        assert_eq!(
            local_cleanup("kan du sende filen spørsmålstegn takk punktum"),
            "Kan du sende filen? Takk."
        );
        assert_eq!(
            local_cleanup("first comma, then second semicolon third"),
            "First, then second; third"
        );
        assert_eq!(local_cleanup("hello new line world"), "Hello\nWorld");
    }

    #[test]
    fn local_cleanup_leaves_plain_text_alone() {
        assert_eq!(
            local_cleanup("Already fine. Really."),
            "Already fine. Really."
        );
        assert_eq!(local_cleanup("  "), "");
    }

    #[test]
    fn short_utterances_respect_the_threshold() {
        let mut settings = AppSettings::default();
        assert!(!is_short_utterance(&settings, "ok"));
        settings.post_process_min_words = 4;
        assert!(is_short_utterance(&settings, "ja takk"));
        assert!(is_short_utterance(&settings, "one two three"));
        assert!(!is_short_utterance(&settings, "one two three four"));
    }

    #[test]
    fn unconfigured_post_processing_is_not_a_failure() {
        let mut settings = AppSettings::default();
        settings.post_process_selected_prompt_id = None;
        assert!(!post_processing_configured(&settings, "hello there"));
        assert!(!post_processing_configured(&settings, "   "));
    }

    #[test]
    fn clipboard_is_bounded() {
        let long = "x".repeat(MAX_CLIPBOARD_CHARS + 500);
        assert_eq!(
            clipboard_for_prompt(&long).chars().count(),
            MAX_CLIPBOARD_CHARS
        );
    }

    #[test]
    fn snippet_tags_expand_verbatim() {
        let snippets = vec![
            snippet("Calendar", "https://cal.example/me?a=1&b=$2"),
            snippet("sig", "Mvh\nFrank"),
        ];
        assert_eq!(
            expand_snippets(
                "Book here: [[snippet: calendar]] thanks\n[[snippet: SIG]]",
                &snippets
            ),
            "Book here: https://cal.example/me?a=1&b=$2 thanks\nMvh\nFrank"
        );
    }

    #[test]
    fn unknown_snippet_is_dropped_and_plain_text_untouched() {
        assert_eq!(expand_snippets("a [[snippet: nope]] b", &[]), "a  b");
        assert_eq!(
            expand_snippets("see [[other]] and [x]", &[]),
            "see [[other]] and [x]"
        );
    }

    #[test]
    fn snippets_are_validated() {
        assert!(validate_snippets(&[snippet("a", "x"), snippet("b", "y")]).is_ok());
        assert!(validate_snippets(&[snippet("", "x")]).is_err());
        assert!(validate_snippets(&[snippet("a[b", "x")]).is_err());
        assert!(validate_snippets(&[snippet("a", "x"), snippet("A", "y")]).is_err());
        assert!(
            validate_snippets(&[snippet("a", &"x".repeat(MAX_SNIPPET_TEXT_CHARS + 1))]).is_err()
        );
    }
}

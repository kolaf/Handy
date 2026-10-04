//! Two-way merge of the user's word list, snippets, learned corrections, own prompts, per-app and per-language rules and a few
//! switches with a shared JSON file, so that the lists can be
//! kept in git and synchronized between machines: `handy --sync-lists PATH` (sent to the running instance).
//!
//! Merging only ever adds. Deletions are not propagated (a deleted entry would come back from the other machine's copy);
//! remove an entry on every machine, or edit the file by hand. When both sides have an entry with the same key but different
//! content (a snippet's text, a correction's replacement), this machine's version wins and is counted as a conflict.

use crate::extras::{is_valid_snippet_name, MAX_SNIPPETS, MAX_SNIPPET_TEXT_CHARS};
use crate::learn::{is_clean_term, MAX_STORED_CORRECTIONS};
use crate::settings::{AppPrompt, Correction, LLMPrompt, LanguageModel, Snippet};
use serde::{Deserialize, Serialize};

const FORMAT_VERSION: u32 = 1;
pub(crate) const MAX_WORDS: usize = 2000;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Lists {
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
    /// The user's own prompts (ids `prompt_...`); the built-in prompts come with the program and are not synced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<LLMPrompt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub app_prompts: Vec<AppPrompt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language_models: Vec<LanguageModel>,
    #[serde(default, skip_serializing_if = "Switches::is_empty")]
    pub switches: Switches,
}

/// A few on/off and choice settings. `None` means "still the default" on this machine: a value from the file is taken only
/// where this machine has the default, and a value of this machine only goes into the file where the file has none.
/// Nothing that was set on purpose is overwritten.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Switches {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_prompts_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_models_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting_speakers: Option<bool>,
}

impl Switches {
    fn is_empty(&self) -> bool {
        *self == Switches::default()
    }
}

pub(crate) const MAX_RULES: usize = 100;
pub(crate) const MAX_PROMPTS: usize = 50;
const MAX_PROMPT_CHARS: usize = 20_000;

#[derive(Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    #[serde(flatten)]
    lists: Lists,
}

#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub words_added: usize,
    pub snippets_added: usize,
    pub corrections_added: usize,
    pub prompts_added: usize,
    pub rules_added: usize,
    pub switches_set: usize,
    /// Same key, different content: this machine's version was kept.
    pub conflicts: Vec<String>,
    /// Entries in the file that failed validation and were ignored.
    pub skipped: usize,
}

impl Report {
    #[cfg(test)]
    pub fn added(&self) -> usize {
        self.words_added
            + self.snippets_added
            + self.corrections_added
            + self.prompts_added
            + self.rules_added
            + self.switches_set
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (n, one, many) in [
            (self.words_added, "word", "words"),
            (self.snippets_added, "snippet", "snippets"),
            (self.corrections_added, "correction", "corrections"),
            (self.prompts_added, "prompt", "prompts"),
            (self.rules_added, "rule", "rules"),
            (self.switches_set, "switch", "switches"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        }
        let mut text = if parts.is_empty() {
            "up to date".to_string()
        } else {
            format!("+{}", parts.join(", +"))
        };
        if !self.conflicts.is_empty() {
            text.push_str(&format!(", {} conflict(s)", self.conflicts.len()));
        }
        text
    }
}

pub(crate) fn word_ok(w: &str) -> bool {
    is_clean_term(w) && w.chars().count() <= 64
}

fn snippet_ok(s: &Snippet) -> bool {
    is_valid_snippet_name(&s.name) && s.text.chars().count() <= MAX_SNIPPET_TEXT_CHARS
}

fn correction_ok(c: &Correction) -> bool {
    is_clean_term(c.wrong.trim()) && is_clean_term(c.right.trim())
}

fn prompt_ok(p: &LLMPrompt) -> bool {
    p.id.starts_with("prompt_")
        && p.id.chars().count() <= 64
        && p.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !p.name.trim().is_empty()
        && p.name.chars().count() <= 100
        && p.prompt.chars().count() <= MAX_PROMPT_CHARS
        && p.examples.chars().count() <= MAX_PROMPT_CHARS
}

fn plain_text(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(|c| c.is_control())
}

fn app_rule_ok(r: &AppPrompt) -> bool {
    !r.app.trim().is_empty()
        && plain_text(&r.app, 100)
        && plain_text(&r.title, 200)
        && !r.prompt_id.is_empty()
}

fn language_rule_ok(r: &LanguageModel) -> bool {
    !r.language.trim().is_empty()
        && plain_text(&r.language, 20)
        && !r.model_id.is_empty()
        && plain_text(&r.model_id, 100)
}

/// Merges `file` into `local`. The result keeps this machine's entries and order, followed by the new ones from the file.
pub fn merge(local: &Lists, file: &Lists) -> (Lists, Report) {
    let mut out = local.clone();
    let mut report = Report::default();

    for w in &file.custom_words {
        let w = w.trim();
        if !word_ok(w) {
            report.skipped += 1;
        } else if out.custom_words.len() >= MAX_WORDS {
            report.skipped += 1;
        } else if !out.custom_words.iter().any(|x| x.eq_ignore_ascii_case(w)) {
            out.custom_words.push(w.to_string());
            report.words_added += 1;
        }
    }

    for s in &file.snippets {
        if !snippet_ok(s) {
            report.skipped += 1;
            continue;
        }
        match out
            .snippets
            .iter()
            .find(|x| x.name.eq_ignore_ascii_case(&s.name))
        {
            Some(mine) => {
                if mine.text != s.text {
                    report.conflicts.push(format!("snippet {}", s.name));
                }
            }
            None if out.snippets.len() >= MAX_SNIPPETS => report.skipped += 1,
            None => {
                out.snippets.push(s.clone());
                report.snippets_added += 1;
            }
        }
    }

    for c in &file.corrections {
        if !correction_ok(c) {
            report.skipped += 1;
            continue;
        }
        match out
            .corrections
            .iter()
            .find(|x| x.wrong.eq_ignore_ascii_case(&c.wrong))
        {
            Some(mine) => {
                if mine.right != c.right {
                    report.conflicts.push(format!("correction {}", c.wrong));
                }
            }
            None if out.corrections.len() >= MAX_STORED_CORRECTIONS => report.skipped += 1,
            None => {
                out.corrections.push(c.clone());
                report.corrections_added += 1;
            }
        }
    }

    for p in &file.prompts {
        if !prompt_ok(p) {
            report.skipped += 1;
            continue;
        }
        match out.prompts.iter().find(|x| x.id == p.id) {
            Some(mine) => {
                if mine.prompt != p.prompt || mine.name != p.name || mine.examples != p.examples {
                    report.conflicts.push(format!("prompt {}", p.name));
                }
            }
            None if out.prompts.len() >= MAX_PROMPTS => report.skipped += 1,
            None => {
                out.prompts.push(p.clone());
                report.prompts_added += 1;
            }
        }
    }

    for r in &file.app_prompts {
        if !app_rule_ok(r) {
            report.skipped += 1;
            continue;
        }
        match out
            .app_prompts
            .iter()
            .find(|x| x.app.eq_ignore_ascii_case(&r.app) && x.title.eq_ignore_ascii_case(&r.title))
        {
            Some(mine) => {
                if mine.prompt_id != r.prompt_id {
                    report.conflicts.push(format!("app rule {}", r.app));
                }
            }
            None if out.app_prompts.len() >= MAX_RULES => report.skipped += 1,
            None => {
                out.app_prompts.push(r.clone());
                report.rules_added += 1;
            }
        }
    }

    for r in &file.language_models {
        if !language_rule_ok(r) {
            report.skipped += 1;
            continue;
        }
        match out
            .language_models
            .iter()
            .find(|x| x.language.eq_ignore_ascii_case(&r.language))
        {
            Some(mine) => {
                if mine.model_id != r.model_id {
                    report.conflicts.push(format!("model for {}", r.language));
                }
            }
            None if out.language_models.len() >= MAX_RULES => report.skipped += 1,
            None => {
                out.language_models.push(r.clone());
                report.rules_added += 1;
            }
        }
    }

    let (mine, theirs) = (&mut out.switches, &file.switches);
    if mine.app_prompts_enabled.is_none() && theirs.app_prompts_enabled.is_some() {
        mine.app_prompts_enabled = theirs.app_prompts_enabled;
        report.switches_set += 1;
    }
    if mine.language_models_enabled.is_none() && theirs.language_models_enabled.is_some() {
        mine.language_models_enabled = theirs.language_models_enabled;
        report.switches_set += 1;
    }
    if mine.meeting_speakers.is_none() && theirs.meeting_speakers.is_some() {
        mine.meeting_speakers = theirs.meeting_speakers;
        report.switches_set += 1;
    }
    if mine.meeting_language.is_none() {
        if let Some(language) = theirs
            .meeting_language
            .as_ref()
            .filter(|l| plain_text(l, 20) && !l.is_empty())
        {
            mine.meeting_language = Some(language.clone());
            report.switches_set += 1;
        }
    }
    (out, report)
}

/// Stable order, so that the file diffs well in git.
fn sorted(lists: &Lists) -> Lists {
    let mut l = lists.clone();
    l.custom_words.sort_by_key(|w| w.to_lowercase());
    l.snippets.sort_by_key(|s| s.name.to_lowercase());
    l.corrections.sort_by_key(|c| c.wrong.to_lowercase());
    l.prompts.sort_by(|a, b| a.id.cmp(&b.id));
    l.app_prompts
        .sort_by_key(|r| (r.app.to_lowercase(), r.title.to_lowercase()));
    l.language_models.sort_by_key(|r| r.language.to_lowercase());
    l
}

pub fn parse(json: &str) -> Result<Lists, String> {
    let file: FileFormat =
        serde_json::from_str(json).map_err(|e| format!("Invalid lists file: {e}"))?;
    if file.version != FORMAT_VERSION {
        return Err(format!(
            "Unsupported lists file version {} (expected {FORMAT_VERSION})",
            file.version
        ));
    }
    Ok(file.lists)
}

pub fn render(lists: &Lists) -> String {
    let file = FileFormat {
        version: FORMAT_VERSION,
        lists: sorted(lists),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("lists serialize");
    text.push('\n');
    text
}

/// Writes through a temporary file so that a crash cannot leave a half-written file in the git checkout.
fn write_atomic(path: &std::path::Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("Cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Cannot replace {}: {e}", path.display()))
}

/// Reads the file (a missing file counts as empty), merges both ways and writes the file. Returns the merged lists for
/// this machine and the report. The file is only rewritten when its content would change.
pub fn sync_file(path: &std::path::Path, local: &Lists) -> Result<(Lists, Report), String> {
    let file_lists = match std::fs::read_to_string(path) {
        Ok(text) => parse(&text)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Lists::default(),
        Err(e) => return Err(format!("Cannot read {}: {e}", path.display())),
    };
    let (merged, report) = merge(local, &file_lists);
    // The file gets everything it had plus what this machine has that it lacked.
    let (for_file, _) = merge(&file_lists, &merged);
    let text = render(&for_file);
    if std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str()) {
        write_atomic(path, &text)?;
    }
    Ok((merged, report))
}

use crate::settings::{get_settings, write_settings};
use log::{info, warn};
use tauri::{AppHandle, Emitter};

/// What this machine contributes. Prompts that come with the program are left out, and switches that are still at their default
/// are `None`.
pub fn lists_of(settings: &crate::settings::AppSettings) -> Lists {
    let default_language = crate::settings::MeetingSettings::default().language;
    Lists {
        custom_words: settings.custom_words.clone(),
        snippets: settings.snippets.clone(),
        corrections: settings.corrections.clone(),
        prompts: settings
            .post_process_prompts
            .iter()
            .filter(|p| p.id.starts_with("prompt_"))
            .cloned()
            .collect(),
        app_prompts: settings.app_prompts.clone(),
        language_models: settings.language_models.clone(),
        switches: Switches {
            app_prompts_enabled: settings.app_prompts_enabled.then_some(true),
            language_models_enabled: settings.language_models_enabled.then_some(true),
            meeting_language: (settings.meeting.language != default_language)
                .then(|| settings.meeting.language.clone()),
            meeting_speakers: settings.meeting.speakers.then_some(true),
        },
    }
}

/// Puts the merged result into the settings (only the parts `lists_of` reads).
pub fn apply(settings: &mut crate::settings::AppSettings, merged: Lists) {
    settings.custom_words = merged.custom_words;
    settings.snippets = merged.snippets;
    settings.corrections = merged.corrections;
    for p in merged.prompts {
        if !settings.post_process_prompts.iter().any(|x| x.id == p.id) {
            settings.post_process_prompts.push(p);
        }
    }
    settings.app_prompts = merged.app_prompts;
    settings.language_models = merged.language_models;
    if let Some(v) = merged.switches.app_prompts_enabled {
        settings.app_prompts_enabled = v;
    }
    if let Some(v) = merged.switches.language_models_enabled {
        settings.language_models_enabled = v;
    }
    if let Some(v) = merged.switches.meeting_speakers {
        settings.meeting.speakers = v;
    }
    if let Some(v) = merged.switches.meeting_language {
        settings.meeting.language = v;
    }
}

/// Entry point for `--sync-lists PATH`. A relative path is resolved against the directory the command was run in.
pub fn run(app: &AppHandle, path: &str, cwd: &str) {
    let mut p = std::path::PathBuf::from(path);
    if p.is_relative() {
        p = std::path::Path::new(cwd).join(p);
    }
    let mut settings = get_settings(app);
    let local = lists_of(&settings);
    match sync_file(&p, &local) {
        Ok((merged, report)) => {
            if merged != local {
                apply(&mut settings, merged);
                write_settings(app, settings);
                let _ = app.emit(
                    "settings-changed",
                    serde_json::json!({ "setting": "lists", "value": report.describe() }),
                );
            }
            info!(
                "Lists sync with {}: {} (skipped {}, conflicts {:?})",
                p.display(),
                report.describe(),
                report.skipped,
                report.conflicts
            );
            let details = format!(
                "File: {}\nSkipped invalid entries: {}\nConflicts (this machine's version was kept): {}",
                p.display(),
                report.skipped,
                if report.conflicts.is_empty() { "none".to_string() } else { report.conflicts.join(", ") }
            );
            crate::activity::log(app, "synced", &report.describe(), &details);
            crate::overlay::show_notice_overlay_unlogged(app, "synced", &report.describe());
        }
        Err(e) => {
            warn!("Lists sync failed: {e}");
            crate::activity::log(app, "synced-failed", "", &e);
            crate::overlay::show_notice_overlay_unlogged(app, "synced-failed", "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: &str, t: &str) -> Snippet {
        Snippet {
            name: n.into(),
            text: t.into(),
        }
    }
    fn c(w: &str, r: &str) -> Correction {
        Correction {
            wrong: w.into(),
            right: r.into(),
            hint: false,
        }
    }

    #[test]
    fn merge_adds_missing_entries_both_ways_without_duplicates() {
        let a = Lists {
            custom_words: vec!["DYST".into(), "Kari".into()],
            ..Default::default()
        };
        let b = Lists {
            custom_words: vec!["dyst".into(), "Ola".into()],
            ..Default::default()
        };
        let (m, r) = merge(&a, &b);
        assert_eq!(m.custom_words, vec!["DYST", "Kari", "Ola"]);
        assert_eq!(r.words_added, 1);
        let (m2, _) = merge(&b, &a);
        assert_eq!(m2.custom_words.len(), 3);
    }

    #[test]
    fn same_key_different_content_keeps_local_and_reports_conflict() {
        let a = Lists {
            snippets: vec![s("sig", "mine")],
            corrections: vec![c("dist", "DYST")],
            ..Default::default()
        };
        let b = Lists {
            snippets: vec![s("Sig", "theirs")],
            corrections: vec![c("dist", "Dyst")],
            ..Default::default()
        };
        let (m, r) = merge(&a, &b);
        assert_eq!(m.snippets[0].text, "mine");
        assert_eq!(m.corrections[0].right, "DYST");
        assert_eq!(r.conflicts.len(), 2);
        assert_eq!(r.added(), 0);
    }

    #[test]
    fn invalid_entries_from_the_file_are_skipped() {
        let bad = Lists {
            custom_words: vec!["ok".into(), "bad<tag>".into(), "".into()],
            snippets: vec![s("bad]name", "x"), s("fine", "text")],
            corrections: vec![c("a", "<b>")],
            ..Default::default()
        };
        let (m, r) = merge(&Lists::default(), &bad);
        assert_eq!(m.custom_words, vec!["ok"]);
        assert_eq!(m.snippets.len(), 1);
        assert!(m.corrections.is_empty());
        assert_eq!(r.skipped, 4);
    }

    #[test]
    fn render_is_sorted_and_round_trips() {
        let l = Lists {
            custom_words: vec!["b".into(), "A".into()],
            snippets: vec![s("z", "1"), s("a", "2")],
            corrections: vec![c("y", "Y"), c("x", "X")],
            ..Default::default()
        };
        let text = render(&l);
        assert!(text.find("\"A\"").unwrap() < text.find("\"b\"").unwrap());
        let back = parse(&text).unwrap();
        assert_eq!(back.custom_words, vec!["A", "b"]);
        assert_eq!(back.snippets[0].name, "a");
    }

    #[test]
    fn unknown_version_is_rejected() {
        assert!(parse(r#"{"version":2}"#).is_err());
        assert!(parse("not json").is_err());
    }

    #[test]
    fn sync_file_creates_then_merges_and_is_stable() {
        let dir = std::env::temp_dir().join(format!("handy-listsync-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lists.json");
        let _ = std::fs::remove_file(&path);

        let laptop = Lists {
            custom_words: vec!["DYST".into()],
            snippets: vec![s("sig", "Hi")],
            ..Default::default()
        };
        let (m, r) = sync_file(&path, &laptop).unwrap();
        assert_eq!(m, laptop);
        assert_eq!(r.added(), 0);

        let desktop = Lists {
            custom_words: vec!["Kari".into()],
            ..Default::default()
        };
        let (m, r) = sync_file(&path, &desktop).unwrap();
        assert_eq!(m.custom_words, vec!["Kari", "DYST"]);
        assert_eq!(m.snippets.len(), 1);
        assert_eq!(r.added(), 2);

        let before = std::fs::read_to_string(&path).unwrap();
        let (_, r) = sync_file(&path, &m).unwrap();
        assert_eq!(r.added(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prompts_rules_and_switches_are_added_and_nothing_is_overwritten() {
        let prompt = |id: &str, text: &str| LLMPrompt {
            id: id.into(),
            name: "Mine".into(),
            prompt: text.into(),
            examples: String::new(),
        };
        let local = Lists {
            prompts: vec![prompt("prompt_1", "local text")],
            app_prompts: vec![AppPrompt {
                app: "slack".into(),
                title: String::new(),
                prompt_id: "informal_message".into(),
            }],
            switches: Switches {
                meeting_speakers: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        let file = Lists {
            prompts: vec![
                prompt("prompt_1", "other text"),
                prompt("prompt_2", "new"),
                prompt("email", "built-in"),
            ],
            app_prompts: vec![
                AppPrompt {
                    app: "Slack".into(),
                    title: String::new(),
                    prompt_id: "email".into(),
                },
                AppPrompt {
                    app: "outlook".into(),
                    title: String::new(),
                    prompt_id: "email".into(),
                },
            ],
            language_models: vec![LanguageModel {
                language: "en".into(),
                model_id: "parakeet-tdt-0.6b-v3".into(),
            }],
            switches: Switches {
                meeting_speakers: Some(false),
                app_prompts_enabled: Some(true),
                meeting_language: Some("no".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (m, r) = merge(&local, &file);
        // a built-in id is not a user prompt, so it is skipped; a different text under the same id is a conflict
        assert_eq!(m.prompts.len(), 2);
        assert_eq!(m.prompts[0].prompt, "local text");
        assert_eq!(r.skipped, 1);
        assert_eq!(m.app_prompts.len(), 2);
        assert_eq!(m.app_prompts[0].prompt_id, "informal_message");
        assert_eq!(m.language_models.len(), 1);
        assert_eq!(m.switches.meeting_speakers, Some(true));
        assert_eq!(m.switches.app_prompts_enabled, Some(true));
        assert_eq!(m.switches.meeting_language.as_deref(), Some("no"));
        assert_eq!(r.conflicts.len(), 2);
        assert_eq!((r.prompts_added, r.rules_added, r.switches_set), (1, 2, 2));
        // the file round trips and a second merge changes nothing
        let (again, r2) = merge(&m, &parse(&render(&m)).unwrap());
        assert_eq!(again, m);
        assert_eq!(r2.added(), 0);
    }

    #[test]
    fn an_old_file_without_the_new_parts_still_loads_and_stays_small() {
        let l =
            parse(r#"{"version":1,"custom_words":["a"],"snippets":[],"corrections":[]}"#).unwrap();
        assert!(l.prompts.is_empty() && l.switches.is_empty());
        let text = render(&l);
        assert!(!text.contains("prompts") && !text.contains("switches"));
    }
}

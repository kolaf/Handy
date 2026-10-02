//! Two-way merge of the user's word list, snippets and learned corrections with a shared JSON file, so that the lists can be
//! kept in git and synchronized between machines: `handy --sync-lists PATH` (sent to the running instance).
//!
//! Merging only ever adds. Deletions are not propagated (a deleted entry would come back from the other machine's copy);
//! remove an entry on every machine, or edit the file by hand. When both sides have an entry with the same key but different
//! content (a snippet's text, a correction's replacement), this machine's version wins and is counted as a conflict.

use crate::extras::{is_valid_snippet_name, MAX_SNIPPETS, MAX_SNIPPET_TEXT_CHARS};
use crate::learn::{is_clean_term, MAX_STORED_CORRECTIONS};
use crate::settings::{Correction, Snippet};
use serde::{Deserialize, Serialize};

const FORMAT_VERSION: u32 = 1;
const MAX_WORDS: usize = 2000;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Lists {
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
}

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
    /// Same key, different content: this machine's version was kept.
    pub conflicts: Vec<String>,
    /// Entries in the file that failed validation and were ignored.
    pub skipped: usize,
}

impl Report {
    pub fn added(&self) -> usize {
        self.words_added + self.snippets_added + self.corrections_added
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (n, one, many) in [
            (self.words_added, "word", "words"),
            (self.snippets_added, "snippet", "snippets"),
            (self.corrections_added, "correction", "corrections"),
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

fn word_ok(w: &str) -> bool {
    is_clean_term(w) && w.chars().count() <= 64
}

fn snippet_ok(s: &Snippet) -> bool {
    is_valid_snippet_name(&s.name) && s.text.chars().count() <= MAX_SNIPPET_TEXT_CHARS
}

fn correction_ok(c: &Correction) -> bool {
    is_clean_term(c.wrong.trim()) && is_clean_term(c.right.trim())
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
    (out, report)
}

/// Stable order, so that the file diffs well in git.
fn sorted(lists: &Lists) -> Lists {
    let mut l = lists.clone();
    l.custom_words.sort_by_key(|w| w.to_lowercase());
    l.snippets.sort_by_key(|s| s.name.to_lowercase());
    l.corrections.sort_by_key(|c| c.wrong.to_lowercase());
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

/// Entry point for `--sync-lists PATH`. A relative path is resolved against the directory the command was run in.
pub fn run(app: &AppHandle, path: &str, cwd: &str) {
    let mut p = std::path::PathBuf::from(path);
    if p.is_relative() {
        p = std::path::Path::new(cwd).join(p);
    }
    let mut settings = get_settings(app);
    let local = Lists {
        custom_words: settings.custom_words.clone(),
        snippets: settings.snippets.clone(),
        corrections: settings.corrections.clone(),
    };
    match sync_file(&p, &local) {
        Ok((merged, report)) => {
            if merged != local {
                settings.custom_words = merged.custom_words;
                settings.snippets = merged.snippets;
                settings.corrections = merged.corrections;
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
            crate::overlay::show_notice_overlay(app, "synced", &report.describe());
        }
        Err(e) => {
            warn!("Lists sync failed: {e}");
            crate::overlay::show_notice_overlay(app, "synced-failed", "");
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
}

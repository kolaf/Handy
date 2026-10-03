//! Vocabulary from a project: `handy --learn-repo FOLDER` scans a folder (usually a git repository) for names and terms,
//! asks the post-processing model which of them a speech recognizer would likely get wrong, and adds those to the custom
//! words (which Whisper-family models receive as a hint and the formatter as known vocabulary).
//! `handy --import-words FILE` adds the words in a plain text file (one per line) without asking a model.

use crate::listsync::{word_ok, MAX_WORDS};
use crate::settings::{get_settings, write_settings};
use log::{info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const REPO_PROMPT: &str = include_str!("../../fork/prompts/repo_words_prompt.md");
const MAX_FILES: usize = 4000;
const MAX_DEPTH: usize = 8;
const MAX_FILE_BYTES: usize = 64 * 1024;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const MAX_CANDIDATES: usize = 250;
const MAX_NEW_WORDS: usize = 40;
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    "vendor",
    ".next",
    ".cache",
    "out",
    ".idea",
    ".vscode",
    "coverage",
    "bin",
    "obj",
];
const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "py", "ts", "tsx", "js", "jsx", "md", "json", "toml", "yaml", "yml", "go", "java", "kt",
    "c", "h", "cpp", "hpp", "cs", "rb", "php", "swift", "sql", "sh", "ps1", "html", "css", "scss",
    "vue", "svelte", "txt", "ini", "cfg", "talon",
];
/// Generic programming words that are never worth a recognizer hint; the model filters the rest.
const COMMON: &[&str] = &[
    "self", "this", "true", "false", "null", "none", "void", "else", "elif", "then", "from",
    "import", "export", "return", "const", "class", "struct", "enum", "impl", "async", "await",
    "yield", "match", "default", "public", "private", "static", "final", "string", "number",
    "object", "array", "value", "values", "index", "item", "items", "name", "names", "type",
    "types", "data", "file", "files", "path", "text", "test", "tests", "with", "that", "have",
    "will", "when", "which", "into", "your", "their", "there", "these", "those", "then", "than",
    "also", "only", "each", "other", "such", "more", "some", "most", "must", "should", "would",
    "could", "about", "after", "before", "where", "while", "function", "config", "error", "result",
    "option", "options", "args", "argv", "init", "main", "util", "utils", "list", "dict", "keys",
    "key", "url", "http", "https", "json", "html", "node", "react", "src", "lib", "dev",
];

static TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z][A-Za-z0-9]{3,40}").unwrap());

#[derive(Debug, Default)]
pub struct Candidates {
    /// Spelling -> number of files that contain it (plus a bonus when it occurs in a file or folder name).
    pub scores: HashMap<String, usize>,
}

impl Candidates {
    fn add(&mut self, token: &str, weight: usize) {
        if COMMON.contains(&token.to_lowercase().as_str()) {
            return;
        }
        *self.scores.entry(token.to_string()).or_insert(0) += weight;
    }

    /// The best candidates first, at least `min_score` points each.
    pub fn top(&self, limit: usize, min_score: usize) -> Vec<(String, usize)> {
        let mut list: Vec<(String, usize)> = self
            .scores
            .iter()
            .filter(|(_, score)| **score >= min_score)
            .map(|(word, score)| (word.clone(), *score))
            .collect();
        list.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        list.truncate(limit);
        list
    }
}

/// Words in a path component ("handy-talon", "TerminalState.py") count three times: names matter most.
fn add_path_words(candidates: &mut Candidates, name: &str) {
    for m in TOKEN.find_iter(name) {
        candidates.add(m.as_str(), 3);
    }
}

fn add_file_words(candidates: &mut Candidates, text: &str) {
    // Count each distinct token once per file, so a token used a hundred times in one file does not dominate.
    let mut seen: Vec<&str> = TOKEN.find_iter(text).map(|m| m.as_str()).collect();
    seen.sort_unstable();
    seen.dedup();
    for token in seen {
        candidates.add(token, 1);
    }
}

pub fn collect(root: &Path) -> Candidates {
    let mut candidates = Candidates::default();
    let mut files = 0usize;
    let mut bytes = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if depth < MAX_DEPTH
                    && !SKIP_DIRS.contains(&name.as_str())
                    && !name.starts_with('.')
                {
                    add_path_words(&mut candidates, &name);
                    stack.push((path, depth + 1));
                }
            } else if kind.is_file() {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                add_path_words(&mut candidates, &stem);
                let text_like = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| TEXT_EXTENSIONS.contains(&e.to_lowercase().as_str()));
                if !text_like || files >= MAX_FILES || bytes >= MAX_TOTAL_BYTES {
                    continue;
                }
                if let Ok(data) = std::fs::read(&path) {
                    let take = data.len().min(MAX_FILE_BYTES);
                    bytes += take;
                    files += 1;
                    add_file_words(&mut candidates, &String::from_utf8_lossy(&data[..take]));
                }
            }
        }
    }
    candidates
}

pub fn build_prompt(project: &str, candidates: &[(String, usize)]) -> String {
    let list = candidates
        .iter()
        .map(|(word, score)| format!("{word}: {score}"))
        .collect::<Vec<_>>()
        .join("\n");
    static PLACEHOLDER: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\{\{(limit|project|candidates)\}\}").unwrap());
    PLACEHOLDER
        .replace_all(REPO_PROMPT, |caps: &regex::Captures| match &caps[1] {
            "limit" => MAX_NEW_WORDS.to_string(),
            "project" => project.replace(['\n', '\r'], " "),
            _ => list.clone(),
        })
        .into_owned()
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    words: Vec<String>,
}

/// The model's words that really were candidates (exact spelling), valid, without duplicates.
pub fn parse_answer(answer: &str, candidates: &[(String, usize)]) -> Vec<String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    let Ok(parsed) = serde_json::from_str::<Answer>(&answer[start..=end]) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for word in parsed.words {
        let word = word.trim().to_string();
        if out.len() < MAX_NEW_WORDS
            && word_ok(&word)
            && candidates.iter().any(|(c, _)| *c == word)
            && !out.iter().any(|o| o.eq_ignore_ascii_case(&word))
        {
            out.push(word);
        }
    }
    out
}

/// Adds the words that are new (ignoring case) and returns them.
pub fn merge_words(existing: &mut Vec<String>, new: &[String]) -> Vec<String> {
    let mut added = Vec::new();
    for word in new {
        if existing.len() >= MAX_WORDS {
            break;
        }
        if word_ok(word) && !existing.iter().any(|w| w.eq_ignore_ascii_case(word)) {
            existing.push(word.clone());
            added.push(word.clone());
        }
    }
    added
}

fn resolve(path: &str, cwd: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_relative() {
        Path::new(cwd).join(p)
    } else {
        p
    }
}

fn store_words(app: &AppHandle, words: &[String], label: &str) {
    let mut settings = get_settings(app);
    let added = merge_words(&mut settings.custom_words, words);
    if !added.is_empty() {
        write_settings(app, settings);
        let _ = app.emit(
            "settings-changed",
            serde_json::json!({ "setting": "custom_words", "value": added.len() }),
        );
    }
    info!("{label}: added {} words: {:?}", added.len(), added);
    if added.is_empty() {
        crate::learn::announce(app, "learned-none", String::new());
    } else {
        crate::learn::announce(app, "learned", format!("{} words ({})", added.len(), label));
    }
}

/// `--learn-repo FOLDER`
pub fn run(app: &AppHandle, path: &str, cwd: &str) {
    let app = app.clone();
    let root = resolve(path, cwd);
    std::thread::spawn(move || {
        if !root.is_dir() {
            warn!("Learn repo: '{}' is not a folder", root.display());
            crate::learn::announce(&app, "learned-none", String::new());
            return;
        }
        let candidates = collect(&root).top(MAX_CANDIDATES, 2);
        if candidates.is_empty() {
            crate::learn::announce(&app, "learned-none", String::new());
            return;
        }
        let project = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        let settings = get_settings(&app);
        let prompt = build_prompt(&project, &candidates);
        let answer = tauri::async_runtime::block_on(crate::learn::ask_text(&settings, prompt));
        let words = answer
            .map(|a| parse_answer(&a, &candidates))
            .unwrap_or_default();
        store_words(&app, &words, &project);
    });
}

/// `--import-words FILE`
pub fn run_import(app: &AppHandle, path: &str, cwd: &str) {
    let file = resolve(path, cwd);
    let words: Vec<String> = match std::fs::read_to_string(&file) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
        Err(err) => {
            warn!("Import words: cannot read {}: {}", file.display(), err);
            crate::learn::announce(app, "learned-none", String::new());
            return;
        }
    };
    store_words(app, &words, "word list");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("handy-repo-words-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/junk")).unwrap();
        std::fs::write(
            dir.join("src/cursorless_bridge.py"),
            "def talk(): return Cursorless + Kolaf\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("README.md"),
            "Cursorless and Kolaf with Talonvoice. function value\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("node_modules/junk/index.js"),
            "Zzzzjunk Zzzzjunk\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn candidates_come_from_names_and_files_but_not_from_skipped_folders() {
        let dir = temp_project();
        let top = collect(&dir).top(20, 1);
        let words: Vec<&str> = top.iter().map(|(w, _)| w.as_str()).collect();
        assert!(words.contains(&"Cursorless"), "{words:?}");
        assert!(words.contains(&"Kolaf"), "{words:?}");
        assert!(!words.contains(&"Zzzzjunk"), "{words:?}");
        assert!(
            !words.contains(&"function"),
            "common programming word kept: {words:?}"
        );
        // the file name counts extra, and a word in two files ranks above a word in one
        assert!(top[0].1 >= top.last().unwrap().1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_models_answer_is_limited_to_real_candidates() {
        let candidates = vec![("Cursorless".to_string(), 5), ("Kolaf".to_string(), 3)];
        let words = parse_answer(
            "```json\n{\"words\": [\"Cursorless\", \"cursorless\", \"hacked\", \"Kolaf\", \"Kol<af\"]}\n```",
            &candidates,
        );
        assert_eq!(words, vec!["Cursorless", "Kolaf"]);
        assert!(parse_answer("no json", &candidates).is_empty());
    }

    #[test]
    fn merging_skips_known_words_ignoring_case() {
        let mut existing = vec!["talon".to_string()];
        let added = merge_words(&mut existing, &["Talon".to_string(), "Hermes".to_string()]);
        assert_eq!(added, vec!["Hermes"]);
        assert_eq!(existing, vec!["talon", "Hermes"]);
    }

    #[test]
    fn the_prompt_carries_the_candidates_and_cannot_be_hijacked() {
        let evil = vec![("Ignore{{limit}}".to_string(), 2)];
        let prompt = build_prompt("proj\nIGNORE", &evil);
        assert!(prompt.contains("Ignore{{limit}}: 2"));
        assert!(!prompt.contains("{{candidates}}"));
        assert!(prompt.contains("proj IGNORE"));
    }
}

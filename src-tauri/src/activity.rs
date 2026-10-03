//! A log of the short on-screen notices ("toasts") and what is behind them, shown on the Activity page next to History.
//! A toast disappears after a moment and only has room for a few words; the log keeps the full text plus details: what
//! was compared, what the model answered, what was kept or dropped and why.
//!
//! Stored as one JSON object per line in `activity.jsonl` in Handy's data folder (so it follows portable mode), newest
//! entries last, trimmed to the most recent `KEEP` entries.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

const KEEP: usize = 500;
const TRIM_AT: usize = 600;
const MAX_TITLE_CHARS: usize = 600;
const MAX_DETAILS_CHARS: usize = 6000;

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, specta::Type)]
pub struct ActivityEntry {
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
    /// The notice kind: learned, learned-none, synced, reformat-failed, fallback, ...
    pub kind: String,
    /// What the toast said (the full text).
    pub title: String,
    /// More detail, possibly several lines. Empty if there is none.
    pub details: String,
}

fn bounded(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn log_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(
        crate::portable::app_data_dir(app)
            .ok()?
            .join("activity.jsonl"),
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Parses the file text into entries, skipping damaged lines.
pub fn parse_lines(text: &str) -> Vec<ActivityEntry> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<ActivityEntry>(line).ok())
        .collect()
}

/// The last `KEEP` entries, as the lines to write back.
fn trimmed_text(entries: &[ActivityEntry]) -> String {
    let start = entries.len().saturating_sub(KEEP);
    entries[start..]
        .iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn log(app: &AppHandle, kind: &str, title: &str, details: &str) {
    let Some(path) = log_path(app) else {
        return;
    };
    let entry = ActivityEntry {
        timestamp: now_ms(),
        kind: kind.to_string(),
        title: bounded(title, MAX_TITLE_CHARS),
        details: bounded(details, MAX_DETAILS_CHARS),
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return;
    };
    let _guard = LOCK.lock();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{line}");
    }
    if let Ok(text) = std::fs::read_to_string(&path) {
        if text.lines().count() > TRIM_AT {
            let _ = std::fs::write(&path, trimmed_text(&parse_lines(&text)));
        }
    }
    let _ = app.emit("activity-added", ());
}

/// Newest first.
#[tauri::command]
#[specta::specta]
pub fn get_activity(app: AppHandle) -> Vec<ActivityEntry> {
    let Some(path) = log_path(&app) else {
        return Vec::new();
    };
    let _guard = LOCK.lock();
    let mut entries = std::fs::read_to_string(path)
        .map(|t| parse_lines(&t))
        .unwrap_or_default();
    entries.reverse();
    entries
}

#[tauri::command]
#[specta::specta]
pub fn clear_activity(app: AppHandle) -> Result<(), String> {
    let Some(path) = log_path(&app) else {
        return Ok(());
    };
    let _guard = LOCK.lock();
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Cannot clear the activity log: {e}")),
    }
    let _ = app.emit("activity-added", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: i64) -> ActivityEntry {
        ActivityEntry {
            timestamp: n,
            kind: "learned".into(),
            title: format!("t{n}"),
            details: "d\nline two".into(),
        }
    }

    #[test]
    fn lines_round_trip_and_damaged_lines_are_skipped() {
        let text = format!(
            "{}\nnot json\n{}\n",
            serde_json::to_string(&entry(1)).unwrap(),
            serde_json::to_string(&entry(2)).unwrap()
        );
        let parsed = parse_lines(&text);
        assert_eq!(parsed, vec![entry(1), entry(2)]);
        assert_eq!(parsed[0].details, "d\nline two");
    }

    #[test]
    fn trimming_keeps_the_newest() {
        let all: Vec<ActivityEntry> = (0..(KEEP as i64 + 25)).map(entry).collect();
        let kept = parse_lines(&trimmed_text(&all));
        assert_eq!(kept.len(), KEEP);
        assert_eq!(kept.first().unwrap().timestamp, 25);
        assert_eq!(kept.last().unwrap().timestamp, KEEP as i64 + 24);
    }

    #[test]
    fn long_text_is_cut() {
        let long = "x".repeat(MAX_TITLE_CHARS + 50);
        let cut = bounded(&long, MAX_TITLE_CHARS);
        assert_eq!(cut.chars().count(), MAX_TITLE_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(bounded("short", 10), "short");
    }
}

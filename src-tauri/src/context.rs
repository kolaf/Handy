//! What the user is doing right now, used to pick the post-processing prompt:
//! - the foreground app (Windows): per-app prompt rules, e.g. Slack -> the informal message prompt;
//! - a one-shot prompt for the next dictation (`handy --use-prompt-once ID`), e.g. "reply to this" from Talon;
//! and a small state file that tells other tools (Talon) whether Handy is recording.

use crate::settings::{get_settings, AppPrompt, AppSettings};
use log::{info, warn};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ONE_SHOT_TTL: Duration = Duration::from_secs(180);
const MAX_RULES: usize = 50;
const MAX_FIELD_CHARS: usize = 80;

static ONE_SHOT: Mutex<Option<(String, Instant)>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppContext {
    /// Program file name, e.g. `slack.exe`.
    pub exe: String,
    pub title: String,
}

#[cfg(windows)]
pub fn foreground() -> Option<AppContext> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len.max(0) as usize]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path_buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        queried.ok()?;
        let path = String::from_utf16_lossy(&path_buf[..size as usize]);
        let exe = path.rsplit(['\\', '/']).next().unwrap_or("").to_string();
        Some(AppContext { exe, title })
    }
}

#[cfg(not(windows))]
pub fn foreground() -> Option<AppContext> {
    None
}

/// `Slack.EXE`, `slack.exe` and `slack` are the same program name.
pub fn normalize_exe(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// The first rule that fits: rules with a title condition are tried before rules for the whole app.
pub fn matching_rule<'a>(rules: &'a [AppPrompt], ctx: &AppContext) -> Option<&'a AppPrompt> {
    let exe = normalize_exe(&ctx.exe);
    let title = ctx.title.to_lowercase();
    let fits = |rule: &&AppPrompt| {
        normalize_exe(&rule.app) == exe
            && (rule.title.trim().is_empty() || title.contains(&rule.title.trim().to_lowercase()))
    };
    rules
        .iter()
        .filter(|r| !r.title.trim().is_empty())
        .find(fits)
        .or_else(|| {
            rules
                .iter()
                .filter(|r| r.title.trim().is_empty())
                .find(fits)
        })
}

pub fn set_one_shot(prompt_id: &str) {
    if let Ok(mut slot) = ONE_SHOT.lock() {
        *slot = Some((prompt_id.to_string(), Instant::now()));
    }
}

/// Takes the one-shot prompt, if one was set recently enough.
pub fn take_one_shot() -> Option<String> {
    let mut slot = ONE_SHOT.lock().ok()?;
    let (id, set_at) = slot.take()?;
    (set_at.elapsed() <= ONE_SHOT_TTL).then_some(id)
}

fn prompt_exists(settings: &AppSettings, id: &str) -> bool {
    settings.post_process_prompts.iter().any(|p| p.id == id)
}

/// Picks the prompt for the dictation that is being post-processed now: a one-shot prompt first, then a per-app rule,
/// else whatever is selected. Changes only the given copy of the settings.
pub fn apply_prompt_choice(settings: &mut AppSettings) {
    if let Some(id) = take_one_shot() {
        if prompt_exists(settings, &id) {
            info!("Prompt for this dictation: '{id}' (one-shot)");
            settings.post_process_selected_prompt_id = Some(id);
            return;
        }
        warn!("One-shot prompt '{id}' does not exist; ignored");
    }
    if !settings.app_prompts_enabled || settings.app_prompts.is_empty() {
        return;
    }
    let Some(ctx) = foreground() else {
        return;
    };
    match matching_rule(&settings.app_prompts, &ctx) {
        Some(rule) if prompt_exists(settings, &rule.prompt_id) => {
            info!(
                "Prompt for this dictation: '{}' (app rule for {} \"{}\")",
                rule.prompt_id, ctx.exe, ctx.title
            );
            settings.post_process_selected_prompt_id = Some(rule.prompt_id.clone());
        }
        Some(rule) => warn!(
            "App rule for {} names an unknown prompt '{}'",
            ctx.exe, rule.prompt_id
        ),
        None => info!(
            "No app rule for {} \"{}\"; using the selected prompt",
            ctx.exe, ctx.title
        ),
    }
}

fn clean_field(s: &str) -> bool {
    s.chars().count() <= MAX_FIELD_CHARS && !s.chars().any(|c| c.is_control())
}

pub fn validate_rules(rules: &[AppPrompt], settings: &AppSettings) -> Result<(), String> {
    if rules.len() > MAX_RULES {
        return Err(format!("At most {MAX_RULES} app rules are allowed"));
    }
    for rule in rules {
        if rule.app.trim().is_empty() || !clean_field(&rule.app) || !clean_field(&rule.title) {
            return Err(format!("Invalid app rule for '{}'", rule.app));
        }
        if !prompt_exists(settings, &rule.prompt_id) {
            return Err(format!("Unknown prompt '{}'", rule.prompt_id));
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_app_prompts(app: tauri::AppHandle, rules: Vec<AppPrompt>) -> Result<(), String> {
    let mut settings = get_settings(&app);
    validate_rules(&rules, &settings)?;
    settings.app_prompts = rules;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_app_prompts_enabled_setting(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.app_prompts_enabled = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

// ---- recording state for other tools --------------------------------------------------------------------------

fn state_file() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(
        std::path::PathBuf::from(home)
            .join(".cache")
            .join("hv")
            .join("handy-state.txt"),
    )
}

fn write_state(recording: bool) {
    let Some(path) = state_file() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let text = format!(
        "{}\n{}\n",
        if recording { "recording" } else { "idle" },
        now
    );
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Keeps `%USERPROFILE%\.cache\hv\handy-state.txt` up to date ("recording" or "idle", then a Unix time) so Talon can
/// stay quiet while Handy listens. While recording the time is refreshed every few seconds so a crash cannot leave a
/// stale "recording" behind (readers ignore old ones).
pub fn start_state_writer(app: &tauri::AppHandle) {
    use tauri::Manager;
    let app = app.clone();
    std::thread::spawn(move || {
        let mut last: Option<bool> = None;
        let mut last_write = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(150));
            let Some(manager) =
                app.try_state::<std::sync::Arc<crate::managers::audio::AudioRecordingManager>>()
            else {
                continue;
            };
            let recording = manager.is_recording();
            if last != Some(recording)
                || (recording && last_write.elapsed() > Duration::from_secs(3))
            {
                write_state(recording);
                last = Some(recording);
                last_write = Instant::now();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(app: &str, title: &str, prompt: &str) -> AppPrompt {
        AppPrompt {
            app: app.into(),
            title: title.into(),
            prompt_id: prompt.into(),
        }
    }

    fn ctx(exe: &str, title: &str) -> AppContext {
        AppContext {
            exe: exe.into(),
            title: title.into(),
        }
    }

    #[test]
    fn program_names_match_with_or_without_exe_and_ignoring_case() {
        let rules = vec![rule("Slack", "", "informal_message")];
        assert!(matching_rule(&rules, &ctx("slack.exe", "x")).is_some());
        assert!(matching_rule(&rules, &ctx("SLACK.EXE", "")).is_some());
        assert!(matching_rule(&rules, &ctx("code.exe", "")).is_none());
    }

    #[test]
    fn a_title_rule_beats_the_rule_for_the_whole_app() {
        let rules = vec![
            rule("msedge.exe", "", "simple"),
            rule("msedge.exe", "gmail", "email"),
        ];
        assert_eq!(
            matching_rule(&rules, &ctx("msedge.exe", "Inbox - Gmail - Edge"))
                .unwrap()
                .prompt_id,
            "email"
        );
        assert_eq!(
            matching_rule(&rules, &ctx("msedge.exe", "Docs"))
                .unwrap()
                .prompt_id,
            "simple"
        );
    }

    #[test]
    fn one_shot_is_used_once() {
        set_one_shot("reply");
        assert_eq!(take_one_shot().as_deref(), Some("reply"));
        assert_eq!(take_one_shot(), None);
    }

    #[test]
    fn rules_are_validated_against_the_prompts() {
        let settings = crate::settings::get_default_settings();
        let known = settings.post_process_prompts[0].id.clone();
        assert!(validate_rules(&[rule("slack.exe", "", &known)], &settings).is_ok());
        assert!(validate_rules(&[rule("slack.exe", "", "nope")], &settings).is_err());
        assert!(validate_rules(&[rule("  ", "", &known)], &settings).is_err());
        assert!(validate_rules(&[rule("a\tb", "", &known)], &settings).is_err());
    }
}

//! What the user is doing right now, used to pick the post-processing prompt:
//! - the foreground app (Windows): per-app prompt rules, e.g. Slack -> the informal message prompt;
//! - a one-shot prompt for the next dictation (`handy --use-prompt-once ID`), e.g. "reply to this" from Talon;
//!
//! It also keeps a small state file that tells other tools (Talon) whether Handy is recording.

use crate::settings::{get_settings, AppPrompt, AppSettings};
use log::{info, warn};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ONE_SHOT_TTL: Duration = Duration::from_secs(180);
const MAX_RULES: usize = 50;
const MAX_FIELD_CHARS: usize = 80;

static ONE_SHOT: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static RECORDING_CONTEXT: Mutex<Option<(AppContext, Instant)>> = Mutex::new(None);
/// A dictation can be long (push-to-talk held, or a toggled recording); a context older than this is not trusted.
const RECORDING_CONTEXT_TTL: Duration = Duration::from_secs(30 * 60);

/// Remembers which app and window a dictation starts in. Called when recording starts: the user may switch windows while
/// the text is being transcribed and formatted, and the text belongs to where they were when they started speaking.
pub fn begin_recording_context() {
    let ctx = foreground();
    if let Ok(mut slot) = RECORDING_CONTEXT.lock() {
        *slot = ctx.map(|c| (c, Instant::now()));
    }
}

/// Takes the context remembered by `begin_recording_context` (once).
pub fn take_recording_context() -> Option<AppContext> {
    let mut slot = RECORDING_CONTEXT.lock().ok()?;
    let (ctx, at) = slot.take()?;
    (at.elapsed() <= RECORDING_CONTEXT_TTL).then_some(ctx)
}

/// Has the user moved to another window since `target`? Only the program and the window are compared, not the title (a
/// title changes as you type, for example "file.txt" becomes "*file.txt"). Unknown on either side counts as "not moved".
pub fn focus_moved(target: Option<&AppContext>, now: Option<&AppContext>) -> bool {
    match (target, now) {
        (Some(t), Some(n)) => {
            normalize_exe(&t.exe) != normalize_exe(&n.exe)
                || (t.window != 0 && n.window != 0 && t.window != n.window)
        }
        _ => false,
    }
}

/// What Handy last pasted as a dictation, so that "scratch dictation" and friends can take it back the way Talon's
/// "scratch that" does: press Backspace once per character. No selecting and no copying, so it also works in
/// terminals (where Ctrl+C would interrupt the running program).
#[derive(Debug, Clone)]
pub struct LastPaste {
    /// The pasted text without the trailing space Handy may add.
    pub text: String,
    /// Backspace presses that remove it: the characters plus the trailing space.
    pub chars: usize,
    /// The window it was pasted into.
    pub window: Option<AppContext>,
    pasted_at: Instant,
}

static LAST_PASTE: Mutex<Option<LastPaste>> = Mutex::new(None);
/// "Scratch" is for taking back what was just said; later than this the cursor has probably been used for other things.
const UNDO_MAX_AGE: Duration = Duration::from_secs(5 * 60);
/// Each character is a key press.
const UNDO_MAX_CHARS: usize = 1500;

/// Remembers a dictation that was just pasted into the window that has focus, and tells other tools (Talon) when.
pub fn note_paste(app: &tauri::AppHandle, text: &str) {
    let trailing = usize::from(get_settings(app).append_trailing_space);
    let last = LastPaste {
        text: text.to_string(),
        chars: text.chars().count() + trailing,
        window: foreground(),
        pasted_at: Instant::now(),
    };
    let chars = last.chars;
    if let Ok(mut slot) = LAST_PASTE.lock() {
        *slot = Some(last);
    }
    write_paste_state(Some(chars));
}

pub fn last_paste() -> Option<LastPaste> {
    LAST_PASTE.lock().ok()?.clone()
}

pub fn clear_last_paste() {
    if let Ok(mut slot) = LAST_PASTE.lock() {
        *slot = None;
    }
    write_paste_state(None);
}

/// How many Backspace presses take the last paste back, or why that is not safe. `now` is the window with focus now
/// (unknown on Wayland and macOS, where only the age is checked).
pub fn undo_plan(
    last: &LastPaste,
    now: Option<&AppContext>,
    age: Duration,
) -> Result<usize, String> {
    if age > UNDO_MAX_AGE {
        return Err(format!(
            "The last dictation was pasted more than {} minutes ago.",
            UNDO_MAX_AGE.as_secs() / 60
        ));
    }
    if last.chars == 0 || last.chars > UNDO_MAX_CHARS {
        return Err(format!(
            "The last dictation is empty or longer than {UNDO_MAX_CHARS} characters."
        ));
    }
    if focus_moved(last.window.as_ref(), now) {
        return Err(format!(
            "The last dictation went into {}, but {} has focus now.",
            describe(last.window.as_ref()),
            describe(now)
        ));
    }
    Ok(last.chars)
}

/// `undo_plan` for the last paste right now.
pub fn current_undo_plan() -> Result<(LastPaste, usize), String> {
    let last = last_paste().ok_or_else(|| "Handy has not pasted a dictation yet.".to_string())?;
    let age = last.pasted_at.elapsed();
    let n = undo_plan(&last, foreground().as_ref(), age)?;
    Ok((last, n))
}

/// `slack.exe "Channel - Slack"` for messages and logs.
pub fn describe(ctx: Option<&AppContext>) -> String {
    ctx.map_or_else(
        || "unknown window".to_string(),
        |c| format!("{} \"{}\"", c.exe, c.title),
    )
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppContext {
    /// Program file name, e.g. `slack.exe`.
    pub exe: String,
    pub title: String,
    /// The window (a window handle on Windows, 0 when unknown); tells two windows of the same program apart.
    pub window: isize,
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
        Some(AppContext {
            exe,
            title,
            window: hwnd.0 as isize,
        })
    }
}

/// The active window under X11 (EWMH `_NET_ACTIVE_WINDOW`): program name from the window's process, title, window id.
/// Wayland does not let applications ask which window is active, so there it is unknown (like on macOS).
#[cfg(target_os = "linux")]
pub fn foreground() -> Option<AppContext> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland")) {
        return None;
    }
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let atom = |name: &str| -> Option<u32> {
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()
            .map(|r| r.atom)
    };

    let active = conn
        .get_property(
            false,
            root,
            atom("_NET_ACTIVE_WINDOW")?,
            AtomEnum::WINDOW,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()?;
    if active == 0 {
        return None;
    }

    let title = conn
        .get_property(
            false,
            active,
            atom("_NET_WM_NAME")?,
            atom("UTF8_STRING")?,
            0,
            256,
        )
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| String::from_utf8_lossy(&r.value).to_string())
        .unwrap_or_default();

    let pid = conn
        .get_property(
            false,
            active,
            atom("_NET_WM_PID")?,
            AtomEnum::CARDINAL,
            0,
            1,
        )
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32()?.next());
    let from_process = pid
        .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/exe")).ok())
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().to_string()));
    // Sandboxed apps hide their process; the window class ("Slack", "firefox") then names the program.
    let exe = from_process.or_else(|| {
        let class = conn
            .get_property(false, active, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 128)
            .ok()?
            .reply()
            .ok()?;
        class_from_wm_class(&class.value)
    })?;
    Some(AppContext {
        exe,
        title,
        window: active as isize,
    })
}

/// `WM_CLASS` holds "instance\0Class\0"; the class (second part) is the better program name.
#[cfg(target_os = "linux")]
fn class_from_wm_class(value: &[u8]) -> Option<String> {
    let mut parts = value.split(|b| *b == 0).filter(|p| !p.is_empty());
    let instance = parts.next()?;
    let class = parts.next().unwrap_or(instance);
    Some(String::from_utf8_lossy(class).to_lowercase())
}

#[cfg(not(any(windows, target_os = "linux")))]
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

/// Terminal emulators: a plain Ctrl+C there interrupts the running program, so copying uses Ctrl+Shift+C.
#[rustfmt::skip]
const TERMINALS: &[&str] = &[
    // Windows
    "windowsterminal", "wt", "conhost", "openconsole", "cmd", "powershell", "pwsh", "mintty", "putty",
    // cross-platform
    "alacritty", "wezterm-gui", "wezterm", "kitty", "warp", "tabby", "hyper", "terminus", "ghostty",
    // Linux (the program name is the process, e.g. gnome-terminal-server)
    "gnome-terminal-server", "gnome-terminal", "konsole", "xterm", "urxvt", "rxvt", "terminator", "tilix",
    "xfce4-terminal", "lxterminal", "qterminal", "mate-terminal", "sakura", "guake", "yakuake", "foot",
    "st", "ptyxis", "blackbox", "kgx", "terminology", "cool-retro-term",
];

pub fn is_terminal_exe(exe: &str) -> bool {
    TERMINALS.contains(&normalize_exe(exe).as_str())
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
///
/// Returns the id of a one-shot prompt that was asked for but is not in the prompt list (the selected prompt is used instead, and the
/// caller says so).
pub fn apply_prompt_choice(
    settings: &mut AppSettings,
    started_in: Option<&AppContext>,
) -> Option<String> {
    let mut missing = None;
    if let Some(id) = take_one_shot() {
        if prompt_exists(settings, &id) {
            info!("Prompt for this dictation: '{id}' (one-shot)");
            settings.post_process_selected_prompt_id = Some(id);
            return None;
        }
        warn!("One-shot prompt '{id}' does not exist; ignored");
        missing = Some(id);
    }
    if !settings.app_prompts_enabled || settings.app_prompts.is_empty() {
        return missing;
    }
    // The app where the dictation started; for actions that do not record (re-run) the app that has focus now.
    let Some(ctx) = started_in.cloned().or_else(foreground) else {
        return missing;
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
    missing
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
pub fn change_paste_focus_guard_setting(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.paste_focus_guard = enabled;
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

/// `%USERPROFILE%\.cache\hv\handy-paste.txt` says when Handy last pasted a dictation that can still be taken back:
/// `<unix milliseconds>` and `<characters>` on two lines, or `none`. Talon compares that time with the time of its own
/// last phrase to decide whether "scratch that" is Talon's or Handy's.
fn write_paste_state(chars: Option<usize>) {
    let Some(state) = state_file() else {
        return;
    };
    let path = state.with_file_name("handy-paste.txt");
    let text = match chars {
        Some(n) => {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis());
            format!("{ms}\n{n}\n")
        }
        None => "none\n".to_string(),
    };
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
            window: 0,
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
    fn moving_to_another_window_is_detected_but_typing_in_the_same_one_is_not() {
        let editor = |title: &str, window: isize| AppContext {
            exe: "code.exe".into(),
            title: title.into(),
            window,
        };
        let start = editor("notes.md", 100);
        assert!(!focus_moved(Some(&start), Some(&editor("*notes.md", 100))));
        assert!(focus_moved(Some(&start), Some(&editor("notes.md", 200))));
        let other = AppContext {
            exe: "slack.exe".into(),
            title: "x".into(),
            window: 300,
        };
        assert!(focus_moved(Some(&start), Some(&other)));
        assert!(!focus_moved(None, Some(&other)));
        assert!(!focus_moved(Some(&start), None));
        assert!(!focus_moved(Some(&editor("a", 0)), Some(&editor("b", 5))));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn wm_class_gives_the_class_name() {
        assert_eq!(
            class_from_wm_class(b"slack\0Slack\0").as_deref(),
            Some("slack")
        );
        assert_eq!(
            class_from_wm_class(b"Navigator\0firefox\0").as_deref(),
            Some("firefox")
        );
        assert_eq!(class_from_wm_class(b"solo\0").as_deref(), Some("solo"));
        assert_eq!(class_from_wm_class(b""), None);
    }

    /// Needs an X display; skipped without one. Makes a window, marks it as the active one the way a window manager
    /// does, and checks that `foreground` reads its title, process and id.
    #[cfg(target_os = "linux")]
    #[test]
    fn reads_the_active_window_from_x11() {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{
            AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass,
        };
        use x11rb::wrapper::ConnectionExt as _;
        let Ok((conn, screen_num)) = x11rb::connect(None) else {
            return;
        };
        if std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland")) {
            return;
        }
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let atom = |name: &str| {
            conn.intern_atom(false, name.as_bytes())
                .unwrap()
                .reply()
                .unwrap()
                .atom
        };
        let (active_atom, name_atom, utf8, pid_atom) = (
            atom("_NET_ACTIVE_WINDOW"),
            atom("_NET_WM_NAME"),
            atom("UTF8_STRING"),
            atom("_NET_WM_PID"),
        );
        let previous = conn
            .get_property(false, root, active_atom, AtomEnum::WINDOW, 0, 1)
            .unwrap()
            .reply()
            .unwrap()
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or(0);

        let window = conn.generate_id().unwrap();
        conn.create_window(
            0,
            window,
            root,
            0,
            0,
            10,
            10,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            window,
            name_atom,
            utf8,
            "Inbox - Slack".as_bytes(),
        )
        .unwrap();
        conn.change_property32(
            PropMode::REPLACE,
            window,
            pid_atom,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .unwrap();
        conn.change_property32(
            PropMode::REPLACE,
            root,
            active_atom,
            AtomEnum::WINDOW,
            &[window],
        )
        .unwrap();
        conn.flush().unwrap();

        let seen = foreground();

        conn.change_property32(
            PropMode::REPLACE,
            root,
            active_atom,
            AtomEnum::WINDOW,
            &[previous],
        )
        .unwrap();
        conn.destroy_window(window).unwrap();
        conn.flush().unwrap();

        let seen = seen.expect("the active window should be found");
        assert_eq!(seen.title, "Inbox - Slack");
        assert_eq!(seen.window, window as isize);
        let own = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(seen.exe, own);
    }

    fn pasted(chars: usize, window: Option<AppContext>) -> LastPaste {
        LastPaste {
            text: "x".repeat(chars),
            chars,
            window,
            pasted_at: Instant::now(),
        }
    }

    #[test]
    fn a_recent_paste_in_the_same_window_can_be_taken_back() {
        let here = AppContext {
            exe: "code.exe".into(),
            title: "a".into(),
            window: 7,
        };
        let last = pasted(40, Some(here.clone()));
        let minute = Duration::from_secs(60);
        assert_eq!(undo_plan(&last, Some(&here), minute), Ok(40));
        // typing changes the title but not the window
        let typed = AppContext {
            title: "*a".into(),
            ..here.clone()
        };
        assert_eq!(undo_plan(&last, Some(&typed), minute), Ok(40));
        // unknown current window (Wayland): only the age is checked
        assert_eq!(undo_plan(&last, None, minute), Ok(40));
    }

    #[test]
    fn an_old_a_moved_or_huge_paste_is_not_taken_back() {
        let here = AppContext {
            exe: "code.exe".into(),
            title: "a".into(),
            window: 7,
        };
        let elsewhere = AppContext {
            exe: "slack.exe".into(),
            title: "b".into(),
            window: 9,
        };
        let last = pasted(40, Some(here.clone()));
        assert!(undo_plan(&last, Some(&here), Duration::from_secs(10 * 60)).is_err());
        assert!(undo_plan(&last, Some(&elsewhere), Duration::from_secs(5)).is_err());
        assert!(undo_plan(&pasted(0, Some(here.clone())), Some(&here), Duration::ZERO).is_err());
        assert!(undo_plan(
            &pasted(5000, Some(here.clone())),
            Some(&here),
            Duration::ZERO
        )
        .is_err());
    }

    #[test]
    fn terminals_are_recognised() {
        assert!(is_terminal_exe("gnome-terminal-server"));
        assert!(is_terminal_exe("konsole"));
        assert!(!is_terminal_exe("firefox"));
        assert!(is_terminal_exe("WindowsTerminal.exe"));
        assert!(is_terminal_exe("pwsh.exe"));
        assert!(!is_terminal_exe("code.exe"));
        assert!(!is_terminal_exe("slack.exe"));
    }

    #[test]
    fn a_one_shot_prompt_that_does_not_exist_is_reported() {
        let mut settings = crate::settings::get_default_settings();
        let selected = settings.post_process_selected_prompt_id.clone();
        set_one_shot("no-such-prompt");
        assert_eq!(
            apply_prompt_choice(&mut settings, None).as_deref(),
            Some("no-such-prompt")
        );
        assert_eq!(settings.post_process_selected_prompt_id, selected);
        set_one_shot("edit");
        assert_eq!(apply_prompt_choice(&mut settings, None), None);
        assert_eq!(
            settings.post_process_selected_prompt_id.as_deref(),
            Some("edit")
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

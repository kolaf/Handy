use crate::TranscriptionCoordinator;
#[cfg(unix)]
use log::debug;
use log::warn;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use signal_hook::consts::SIGUSR1;
#[cfg(unix)]
use signal_hook::consts::SIGUSR2;
#[cfg(unix)]
use signal_hook::iterator::Signals;
#[cfg(unix)]
use std::thread;

/// Send a transcription input to the coordinator.
/// Used by signal handlers, CLI flags, and any other external trigger.
pub fn send_transcription_input(app: &AppHandle, binding_id: &str, source: &str) {
    if let Some(c) = app.try_state::<TranscriptionCoordinator>() {
        c.send_external_input(binding_id, source);
    } else {
        warn!("TranscriptionCoordinator not initialized");
    }
}

/// Returns the value of `--name value` or `--name=value` from raw process arguments.
pub fn flag_value(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
        if arg == name {
            return iter.next().cloned();
        }
    }
    None
}

/// Runs a shortcut action by binding id, exactly as if its hotkey had been pressed.
/// Used by CLI flags for actions that are not part of the transcription pipeline.
pub fn run_action(app: &AppHandle, binding_id: &str, source: &str) {
    match crate::actions::ACTION_MAP.get(binding_id) {
        Some(action) => action.start(app, binding_id, source),
        None => warn!("No action registered for '{}'", binding_id),
    }
}

/// Listen for Unix signals that remotely toggle transcription.
///
/// SIGUSR2 toggles plain transcription on all Unix platforms. SIGUSR1
/// (transcription with post-processing) is only handled on macOS: on Linux,
/// WebKitGTK's JavaScriptCore garbage collector sends SIGUSR1 to its own
/// threads to suspend them, so handling it caused phantom recordings on every
/// GC cycle (#1660). Linux users should use `handy --toggle-post-process`
/// instead.
#[cfg(unix)]
pub fn setup_signal_handler(app_handle: AppHandle) {
    #[cfg(target_os = "macos")]
    let mut signals =
        Signals::new([SIGUSR1, SIGUSR2]).expect("failed to register transcription signal handlers");
    #[cfg(not(target_os = "macos"))]
    let mut signals =
        Signals::new([SIGUSR2]).expect("failed to register transcription signal handlers");
    #[cfg(target_os = "macos")]
    debug!("Signal handlers registered (SIGUSR1, SIGUSR2)");
    #[cfg(not(target_os = "macos"))]
    debug!("Signal handler registered (SIGUSR2; SIGUSR1 is left to WebKitGTK)");
    thread::spawn(move || {
        for sig in signals.forever() {
            let (binding_id, signal_name) = match sig {
                #[cfg(target_os = "macos")]
                SIGUSR1 => ("transcribe_with_post_process", "SIGUSR1"),
                SIGUSR2 => ("transcribe", "SIGUSR2"),
                _ => continue,
            };
            debug!("Received {signal_name}");
            send_transcription_input(&app_handle, binding_id, signal_name);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::flag_value;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flag_value_reads_both_forms() {
        let a = args(&["handy", "--set-prompt", "email", "--set-language=no"]);
        assert_eq!(flag_value(&a, "--set-prompt").as_deref(), Some("email"));
        assert_eq!(flag_value(&a, "--set-language").as_deref(), Some("no"));
        assert_eq!(flag_value(&a, "--cancel"), None);
    }

    #[test]
    fn flag_value_without_a_value_is_none() {
        assert_eq!(
            flag_value(&args(&["handy", "--set-prompt"]), "--set-prompt"),
            None
        );
    }
}

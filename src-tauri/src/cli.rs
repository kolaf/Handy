use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "handy", about = "Handy - Speech to Text")]
pub struct CliArgs {
    /// Start with the main window hidden
    #[arg(long)]
    pub start_hidden: bool,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,

    /// Toggle transcription on/off (sent to running instance)
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance)
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance)
    #[arg(long)]
    pub cancel: bool,

    /// Swap the selected language with the alternate language (sent to running instance)
    #[arg(long)]
    pub swap_language: bool,

    /// Re-run the last dictation with the next post-processing prompt (sent to running instance)
    #[arg(long)]
    pub rerun: bool,

    /// Open the numbered prompt picker (sent to running instance)
    #[arg(long)]
    pub prompt_picker: bool,

    /// Replace the selected text with its reformatted version, using the selected prompt (sent to running instance)
    #[arg(long)]
    pub reformat: bool,

    /// Learn from the selected, corrected text (sent to running instance)
    #[arg(long)]
    pub learn: bool,

    /// Merge the word list, snippets and learned corrections with a shared JSON file, both ways (sent to running instance)
    #[arg(long, value_name = "FILE")]
    pub sync_lists: Option<String>,

    /// Paste the most recent dictation again (sent to running instance)
    #[arg(long)]
    pub paste_last: bool,

    /// Select a post-processing prompt by id (sent to running instance)
    #[arg(long, value_name = "ID")]
    pub set_prompt: Option<String>,

    /// Set the dictation language, e.g. `no`, `en`, `auto` (sent to running instance)
    #[arg(long, value_name = "CODE")]
    pub set_language: Option<String>,

    /// Enable debug mode with verbose logging
    #[arg(long)]
    pub debug: bool,

    /// Transcribe this WAV (16 kHz mono) headlessly and exit. Runs the same
    /// batch transcription path as the app — no mic, no VAD, no download
    /// (the model must already be installed).
    #[arg(short = 'f', long, value_name = "WAV")]
    pub transcribe_file: Option<PathBuf>,

    /// Model id to load for --transcribe-file (default: the selected model).
    #[arg(long)]
    pub model: Option<String>,

    /// Hard-select the compute device for --transcribe-file by its registry
    /// index (see --list-devices). Omit to use the persisted accelerator
    /// setting. transcribe-cpp (whisper-family) models only.
    #[arg(long, value_name = "N")]
    pub device_index: Option<usize>,

    /// List the transcribe-cpp compute devices (with indices) and exit.
    #[arg(long)]
    pub list_devices: bool,

    /// List the available models (with ids) and exit. Pass an id to --model.
    /// Honors --json for machine-readable output.
    #[arg(long)]
    pub list_models: bool,

    /// Repeat the transcription N times (best_ms reports the fastest run).
    #[arg(long, value_name = "N")]
    pub repeat: Option<usize>,

    /// Emit --transcribe-file results as JSON.
    #[arg(long)]
    pub json: bool,
}

#[cfg(test)]
mod tests {
    use super::CliArgs;
    use clap::Parser;

    #[test]
    fn switch_flags_parse() {
        let args =
            CliArgs::try_parse_from(["handy", "--swap-language", "--prompt-picker"]).unwrap();
        assert!(args.swap_language);
        assert!(args.prompt_picker);
        assert!(!CliArgs::try_parse_from(["handy"]).unwrap().swap_language);
    }

    #[test]
    fn set_flags_take_values() {
        let args = CliArgs::try_parse_from(["handy", "--set-prompt", "email", "--set-language=no"])
            .unwrap();
        assert_eq!(args.set_prompt.as_deref(), Some("email"));
        assert_eq!(args.set_language.as_deref(), Some("no"));
        assert!(CliArgs::try_parse_from(["handy", "--set-prompt"]).is_err());
    }
}

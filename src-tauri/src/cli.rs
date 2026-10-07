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

    /// Write meeting minutes from audio files; several parts of one session separated by ';' (sent to running instance)
    #[arg(long, value_name = "FILES")]
    pub meeting_minutes: Option<String>,

    /// Language of the meeting recording and minutes, e.g. no or en (with --meeting-minutes)
    #[arg(long, value_name = "CODE")]
    pub meeting_language: Option<String>,

    /// Write minutes for the latest recording in the recorder's folder (OBS Studio...) (sent to running instance)
    #[arg(long)]
    pub meeting_latest: bool,

    /// With --meeting-latest: use only the newest file, not the files that belong to the same recording
    #[arg(long)]
    pub meeting_single: bool,

    /// With --meeting-latest: look in this folder instead of the one set on the Meetings page
    #[arg(long, value_name = "FOLDER")]
    pub meeting_folder: Option<String>,

    /// Speech model for the meeting transcription only, e.g. nb or parakeet (with --meeting-minutes)
    #[arg(long, value_name = "NAME")]
    pub meeting_model: Option<String>,

    /// Put the meeting in the context of a SilverBullet project (a page with tags: project): its notes and tasks inform the minutes, and
    /// the minutes are written to a new page there with proposed tasks (with --meeting-minutes or --meeting-latest)
    #[arg(long, value_name = "NAME")]
    pub meeting_project: Option<String>,

    /// Identify the speakers in the minutes (with --meeting-minutes or --meeting-latest): the audio is sent to the
    /// post-processing endpoint's diarizing model instead of being transcribed locally
    #[arg(long)]
    pub meeting_speakers: bool,

    /// Switch the post-processing language model: `local` (a llama-server on this computer), `cloud` (the custom
    /// provider, e.g. a hosted gateway) or part of a provider's name (sent to running instance)
    #[arg(long, value_name = "NAME")]
    pub set_llm: Option<String>,

    /// Switch the speech model by (part of) its name or id, e.g. `parakeet` (sent to running instance)
    #[arg(long, value_name = "NAME")]
    pub set_model: Option<String>,

    /// Open the numbered model picker (sent to running instance)
    #[arg(long)]
    pub model_picker: bool,

    /// Open the numbered prompt picker (sent to running instance)
    #[arg(long)]
    pub prompt_picker: bool,

    /// Replace the selected text with its reformatted version, using the selected prompt (sent to running instance)
    #[arg(long)]
    pub reformat: bool,

    /// Delete the last dictation if it is the text just before the cursor (sent to running instance)
    #[arg(long)]
    pub scratch_last: bool,

    /// Replace the last dictation by the same recording processed again with this prompt (sent to running instance)
    #[arg(long, value_name = "ID")]
    pub redo_with: Option<String>,

    /// Use this prompt for the next dictation only (sent to running instance)
    #[arg(long, value_name = "ID")]
    pub use_prompt_once: Option<String>,

    /// Run the selection (or, with nothing selected, the last dictation) through this prompt and replace it
    #[arg(long, value_name = "ID")]
    pub transform: Option<String>,

    /// Add project-specific words from a folder (a repository) to the custom words, with the help of the model
    #[arg(long, value_name = "FOLDER")]
    pub learn_repo: Option<String>,

    /// Add the words in a text file (one per line) to the custom words
    #[arg(long, value_name = "FILE")]
    pub import_words: Option<String>,

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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::{is_microphone_access_denied, is_no_input_device_error, VadPolicy};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::model::ModelManager;
use crate::managers::transcription::StreamWorkKind;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings, OverlayStyle, APPLE_INTELLIGENCE_PROVIDER_ID};
use crate::shortcut;
use crate::tray::{set_tray_state, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
};
use crate::TranscriptionCoordinator;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

/// Drop guard that finishes the transcription pipeline, including immediate
/// model unloading on early exits.
struct FinishGuard(AppHandle, Arc<TranscriptionManager>);
impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.1.maybe_unload_immediately("transcription session");
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
        // The pipeline just freed its large transient buffers (captured PCM,
        // WAV copy, engine scratch); hand the cached pages back to the OS so
        // they don't sit in malloc arenas until they get swapped out (#1792).
        crate::memory::trim_freed_memory();
    }
}

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction {
    post_process: bool,
}

/// Field name for structured output JSON schema
const TRANSCRIPTION_FIELD: &str = "transcription";

/// Strip invisible Unicode characters that some LLMs may insert
fn strip_invisible_chars(s: &str) -> String {
    s.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "")
}

/// Strip a leading `<think>...</think>` block. Some endpoints can't disable
/// reasoning, and some local servers put the reasoning text into `content`
/// instead of a separate field — without this the user would get the model's
/// chain of thought pasted along with the cleaned transcription.
fn strip_think_block(s: &str) -> &str {
    if let Some(rest) = s.trim_start().strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            return rest[end + "</think>".len()..].trim_start();
        }
    }
    s
}

/// Build a system prompt from the user's prompt template.
/// Removes `${output}` placeholder since the transcription is sent as the user message.
fn build_system_prompt(prompt_template: &str) -> String {
    prompt_template.replace("${output}", "").trim().to_string()
}

/// Returns `true` when a transcription has no meaningful content to
/// post-process (empty or whitespace-only). Used to skip the post-processing
/// LLM call when nothing was actually transcribed, which would otherwise make
/// the model reply with an error message such as "you need to provide the
/// transcription".
fn is_blank_transcription(transcription: &str) -> bool {
    transcription.trim().is_empty()
}

async fn complete_unless_cancelled<F, C>(operation: F, is_cancelled: C) -> Option<F::Output>
where
    F: Future,
    C: Fn() -> bool,
{
    tokio::pin!(operation);

    loop {
        if is_cancelled() {
            return None;
        }

        if let Ok(result) =
            tokio::time::timeout(CANCELLATION_POLL_INTERVAL, operation.as_mut()).await
        {
            return Some(result);
        }
    }
}

fn should_use_streaming_overlay(style: OverlayStyle, is_streaming: bool) -> bool {
    style == OverlayStyle::Live && is_streaming
}

pub(crate) async fn post_process_transcription(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    if is_blank_transcription(transcription) {
        debug!("Post-processing skipped because the transcription is empty");
        return None;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return None;
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return None;
    }

    let selected_prompt_id = match &settings.post_process_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            debug!("Post-processing skipped because no prompt is selected");
            return None;
        }
    };

    let prompt = match settings
        .post_process_prompts
        .iter()
        .find(|prompt| prompt.id == selected_prompt_id)
    {
        Some(prompt) => prompt.prompt.clone(),
        None => {
            debug!(
                "Post-processing skipped because prompt '{}' was not found",
                selected_prompt_id
            );
            return None;
        }
    };

    if prompt.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return None;
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Ask these providers to skip reasoning/thinking — post-processing rarely
    // benefits from it and it adds seconds of latency. llm_client picks the
    // field the endpoint understands and retries without it if rejected.
    let disable_reasoning = matches!(provider.id.as_str(), "custom" | "openrouter");

    if provider.supports_structured_output {
        debug!("Using structured outputs for provider '{}'", provider.id);

        let system_prompt = build_system_prompt(&prompt);
        let user_content = transcription.to_string();

        // Handle Apple Intelligence separately since it uses native Swift APIs
        if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            {
                if !apple_intelligence::check_apple_intelligence_availability() {
                    debug!(
                        "Apple Intelligence selected but not currently available on this device"
                    );
                    return None;
                }

                let token_limit = model.trim().parse::<i32>().unwrap_or(0);
                return match apple_intelligence::process_text_with_system_prompt(
                    &system_prompt,
                    &user_content,
                    token_limit,
                ) {
                    Ok(result) => {
                        if result.trim().is_empty() {
                            debug!("Apple Intelligence returned an empty response");
                            None
                        } else {
                            let result = strip_invisible_chars(&result);
                            debug!(
                                "Apple Intelligence post-processing succeeded. Output length: {} chars",
                                result.len()
                            );
                            Some(result)
                        }
                    }
                    Err(err) => {
                        error!("Apple Intelligence post-processing failed: {}", err);
                        None
                    }
                };
            }

            #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
            {
                debug!("Apple Intelligence provider selected on unsupported platform");
                return None;
            }
        }

        // Define JSON schema for transcription output
        let json_schema = serde_json::json!({
            "type": "object",
            "properties": {
                (TRANSCRIPTION_FIELD): {
                    "type": "string",
                    "description": "The cleaned and processed transcription text"
                }
            },
            "required": [TRANSCRIPTION_FIELD],
            "additionalProperties": false
        });

        match crate::llm_client::send_chat_completion_with_schema(
            &provider,
            api_key.clone(),
            &model,
            user_content,
            Some(system_prompt),
            Some(json_schema),
            disable_reasoning,
        )
        .await
        {
            Ok(Some(content)) => {
                // Parse the JSON response to extract the transcription field
                let content = strip_think_block(&content);
                match serde_json::from_str::<serde_json::Value>(content) {
                    Ok(json) => {
                        if let Some(transcription_value) =
                            json.get(TRANSCRIPTION_FIELD).and_then(|t| t.as_str())
                        {
                            let result = strip_invisible_chars(transcription_value);
                            debug!(
                                "Structured output post-processing succeeded for provider '{}'. Output length: {} chars",
                                provider.id,
                                result.len()
                            );
                            return Some(result);
                        } else {
                            error!("Structured output response missing 'transcription' field");
                            return Some(strip_invisible_chars(content));
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse structured output JSON: {}. Returning raw content.",
                            e
                        );
                        return Some(strip_invisible_chars(content));
                    }
                }
            }
            Ok(None) => {
                error!("LLM API response has no content");
                return None;
            }
            Err(e) => {
                warn!(
                    "Structured output failed for provider '{}': {}. Falling back to legacy mode.",
                    provider.id, e
                );
                // Fall through to legacy mode below
            }
        }
    }

    // Legacy mode: Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    match crate::llm_client::send_chat_completion(
        &provider,
        api_key,
        &model,
        processed_prompt,
        disable_reasoning,
    )
    .await
    {
        Ok(Some(content)) => {
            let content = strip_invisible_chars(strip_think_block(&content));
            debug!(
                "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                provider.id,
                content.len()
            );
            Some(content)
        }
        Ok(None) => {
            error!("LLM API response has no content");
            None
        }
        Err(e) => {
            error!(
                "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                provider.id,
                e
            );
            None
        }
    }
}

pub(crate) struct ProcessedTranscription {
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
}

/// Most words one dictation may add to the vocabulary, and the longest a word may be.
const MAX_VOCAB_COMMANDS: usize = 5;
const MAX_VOCAB_WORD_CHARS: usize = 64;

/// Splits `[[vocab: WORD]]` command tags from the post-processed text.
///
/// The post-processing prompt can append these tags when the speaker spells out a word
/// or asks to add it to the vocabulary. The text comes from a model reading speech, so
/// the words are validated: one line, bounded length, no brackets or control characters,
/// at most [`MAX_VOCAB_COMMANDS`] per dictation. Returns the text without the tags and
/// the accepted words.
pub(crate) fn extract_vocab_commands(text: &str) -> (String, Vec<String>) {
    static TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[\s*vocab\s*:([^\]\n]*)\]\]").unwrap());
    let mut words: Vec<String> = Vec::new();
    for capture in TAG.captures_iter(text) {
        let word = capture[1]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim();
        let valid = !word.is_empty()
            && word.chars().count() <= MAX_VOCAB_WORD_CHARS
            && !word.chars().any(|c| c.is_control() || c == '[' || c == ']');
        if valid
            && words.len() < MAX_VOCAB_COMMANDS
            && !words.iter().any(|w| w.eq_ignore_ascii_case(word))
        {
            words.push(word.to_string());
        }
    }
    // Tags sit on their own trailing line; drop the line break they leave behind.
    (TAG.replace_all(text, "").trim_end().to_string(), words)
}

/// Adds `words` to the custom words list (skipping ones already there, ignoring case).
fn learn_vocabulary(app: &AppHandle, words: &[String]) {
    let mut settings = get_settings(app);
    let mut added: Vec<String> = Vec::new();
    for word in words {
        if !settings
            .custom_words
            .iter()
            .any(|known| known.eq_ignore_ascii_case(word))
        {
            settings.custom_words.push(word.clone());
            added.push(word.clone());
        }
    }
    if added.is_empty() {
        return;
    }
    log::info!("Vocabulary command: added {:?}", added);
    crate::settings::write_settings(app, settings);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "setting": "custom_words", "value": added }),
    );
    // The overlay is still finishing this dictation; show the notice once it has faded.
    let handle = app.clone();
    let shown = added.join(", ");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        crate::overlay::show_notice_overlay(&handle, "vocab", &shown);
    });
}

pub(crate) async fn process_transcription_output(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
) -> ProcessedTranscription {
    process_transcription_output_in(app, transcription, post_process, None).await
}

/// `started_in`: the app and window the dictation was started in (the prompt rules and the `${app}` and `${title}`
/// variables use it); without it the app that has focus now is used.
pub(crate) async fn process_transcription_output_in(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
    started_in: Option<&crate::context::AppContext>,
) -> ProcessedTranscription {
    // "update journal": this dictation goes into today's SilverBullet journal page instead of being pasted.
    if post_process && crate::journal::take_session() {
        crate::journal::handle_dictation(app, transcription).await;
        return ProcessedTranscription {
            final_text: String::new(),
            post_processed_text: None,
            post_process_prompt: None,
        };
    }
    let mut settings = get_settings(app);
    if post_process {
        // A one-shot prompt ("reply to this") or a per-app rule may replace the selected prompt for this dictation.
        if let Some(missing) = crate::context::apply_prompt_choice(&mut settings, started_in) {
            crate::learn::announce_with(
                app,
                "prompt-missing",
                missing.clone(),
                format!(
                    "The prompt '{missing}' is not in your prompt list, so the selected prompt was used instead. Install the built-in prompts (fork/scripts/install-prompts.py, with Handy closed) or add a prompt with that id."
                ),
            );
        }
        // An edit-style prompt works on the copied text: with nothing on the clipboard there is nothing to edit.
        if crate::extras::prompt_uses_clipboard(&settings) {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            let copied = app.clipboard().read_text().unwrap_or_default();
            info!(
                "The prompt works on the clipboard: {} characters copied",
                copied.chars().count()
            );
            if copied.trim().is_empty() {
                crate::learn::announce_with(
                    app,
                    "edit-none",
                    String::new(),
                    "The selected prompt works on the text you copied, but the clipboard is empty. Select the text first (for \"edit this\" it is copied for you), then speak the change.".to_string(),
                );
                return ProcessedTranscription {
                    final_text: String::new(),
                    post_processed_text: None,
                    post_process_prompt: None,
                };
            }
        }
    }
    // Learned corrections (see learn.rs) fix known mishearings before anything else sees the text.
    let heard = crate::learn::apply_corrections(transcription, &settings.corrections);
    let mut final_text = heard.clone();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    if post_process && crate::extras::skips_model(&settings, &final_text) {
        // Too short to be worth a model call: basic local cleanup only.
        final_text = crate::extras::local_cleanup(&final_text);
        if final_text != transcription {
            post_processed_text = Some(final_text.clone());
        }
    } else if post_process {
        // Prompt variables (${clipboard} etc.) are filled in on a copy so the stored prompt
        // text, which goes into the history, never contains clipboard contents.
        let prompt_settings =
            crate::extras::expand_prompt_variables(app, settings.clone(), started_in);
        if let Some(processed_text) =
            post_process_transcription(&prompt_settings, &final_text).await
        {
            let (processed_text, vocab_words) = extract_vocab_commands(&processed_text);
            if !vocab_words.is_empty() {
                learn_vocabulary(app, &vocab_words);
            }
            let processed_text =
                crate::extras::expand_snippets(&processed_text, &settings.snippets);
            post_processed_text = Some(processed_text.clone());
            final_text = processed_text;

            if let Some(prompt_id) = &settings.post_process_selected_prompt_id {
                if let Some(prompt) = settings
                    .post_process_prompts
                    .iter()
                    .find(|prompt| &prompt.id == prompt_id)
                {
                    post_process_prompt = Some(prompt.prompt.clone());
                }
            }
        } else if crate::extras::post_processing_configured(&settings, &final_text) {
            // The request failed (offline, timeout, error): keep the dictation usable with
            // local cleanup, and say so.
            final_text = crate::extras::local_cleanup(&final_text);
            post_processed_text = Some(final_text.clone());
            crate::extras::notify_fallback(app);
        }
    }

    ProcessedTranscription {
        final_text,
        post_processed_text,
        post_process_prompt,
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);
        // Where the user is when they start speaking; see `context::begin_recording_context`.
        crate::context::begin_recording_context();
        // "update journal": is this recording the journal dictation?
        crate::journal::begin_session(binding_id);

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        let rm = app.state::<Arc<AudioRecordingManager>>();

        // Load ASR model and VAD model in parallel
        let kickoff_started = Instant::now();
        tm.initiate_model_load();
        let rm_clone = Arc::clone(&rm);
        std::thread::spawn(move || {
            if let Err(e) = rm_clone.preload_vad() {
                debug!("VAD pre-load failed: {}", e);
            }
        });
        let kickoff_elapsed = kickoff_started.elapsed();

        // Don't open the mic if nothing can transcribe the recording; the load
        // kicked off above fails and reports why.
        if !tm.is_model_loaded() {
            let selected_model = get_settings(app).selected_model;
            if let Err(e) = app
                .state::<Arc<ModelManager>>()
                .get_model_path(&selected_model)
            {
                warn!("Not starting recording: no model can transcribe it ({})", e);
                return;
            }
        }

        let binding_id = binding_id.to_string();
        let tray_started = Instant::now();
        set_tray_state(app, TrayIconState::Recording);
        let tray_elapsed = tray_started.elapsed();

        // Get the microphone mode to determine audio feedback timing
        let plan_started = Instant::now();
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;

        let selected_model_info = app
            .state::<Arc<ModelManager>>()
            .get_model_info(&settings.selected_model);

        // Use the app-facing model capability as the single pre-recording source
        // for live streaming decisions. Unknown support is represented as false
        // until the model registry is updated by discovery or runtime load.
        let model_supports_streaming = selected_model_info
            .as_ref()
            .map(|m| m.supports_streaming)
            .unwrap_or(false);
        let vad_policy = if !settings.vad_enabled {
            VadPolicy::Disabled
        } else if model_supports_streaming {
            VadPolicy::Streaming
        } else {
            VadPolicy::Offline
        };
        if model_supports_streaming {
            tm.start_stream();
        }
        let plan_elapsed = plan_started.elapsed();

        // Sizing the overlay follows the same advertised capability. A model that
        // doesn't stream (or whose capability is not known yet) gets the compact
        // pill instead of an oversized transparent live window.
        let overlay_started = Instant::now();
        crate::overlay::emit_recording_caption(app, self.post_process);
        match settings.overlay_style {
            OverlayStyle::Live if model_supports_streaming => utils::show_streaming_overlay(app),
            OverlayStyle::Live | OverlayStyle::Minimal => show_recording_overlay(app),
            OverlayStyle::None => {} // show_overlay_state no-ops on None anyway
        }
        // Everything above runs before capture can begin, so each span here is
        // added keypress->capture latency.
        debug!(
            "start-path pre-recording steps: model_kickoff={:?} tray={:?} settings+stream_plan={:?} overlay={:?}",
            kickoff_elapsed,
            tray_elapsed,
            plan_elapsed,
            overlay_started.elapsed()
        );
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_error: Option<String> = None;
        let recording_start_time = Instant::now();
        match rm.try_start_recording(&binding_id, vad_policy) {
            Ok(readiness) => {
                debug!(
                    "Recording request accepted in {:?}; waiting for first microphone samples",
                    recording_start_time.elapsed()
                );
                let generation = readiness.generation();
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    if !readiness.wait() {
                        debug!("Microphone readiness wait ended without receiving samples");
                        return;
                    }

                    // Development-only preview hook for evaluating the brief
                    // arming animation on hardware that normally starts too fast
                    // to make it visible.
                    #[cfg(debug_assertions)]
                    if let Ok(delay_ms) = std::env::var("HANDY_DEBUG_MIC_READY_DELAY_MS")
                        .unwrap_or_default()
                        .parse::<u64>()
                    {
                        let delay_ms = delay_ms.min(10_000);
                        if delay_ms > 0 {
                            debug!("Delaying microphone-ready cue by {delay_ms}ms for UI preview");
                            std::thread::sleep(Duration::from_millis(delay_ms));
                        }
                    }

                    if !rm_clone.is_recording_readiness_current(generation) {
                        debug!("Microphone became ready for an inactive recording");
                        return;
                    }

                    debug!("Microphone is receiving samples; recording is ready");
                    utils::emit_recording_ready(&app_clone);

                    // The start chime is a readiness cue, so it must follow the
                    // first real input callback rather than Stream::play() or a
                    // fixed delay. The helper returns immediately when feedback
                    // is disabled; mute still follows the same readiness point.
                    if rm_clone.is_recording_readiness_current(generation) {
                        play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    }
                    if rm_clone.is_recording_readiness_current(generation) {
                        rm_clone.apply_mute();
                    }
                });
            }
            Err(e) => {
                debug!("Failed to start recording: {}", e);
                recording_error = Some(e);
            }
        }

        if recording_error.is_none() {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        } else {
            // Starting failed (for example due to blocked microphone permissions).
            // Revert UI state so we don't stay stuck in the recording overlay.
            tm.cancel_stream();
            utils::hide_recording_overlay(app);
            set_tray_state(app, TrayIconState::Idle);
            if let Some(err) = recording_error {
                let error_type = if is_microphone_access_denied(&err) {
                    "microphone_permission_denied"
                } else if is_no_input_device_error(&err) {
                    "no_input_device"
                } else {
                    "unknown"
                };
                let _ = app.emit(
                    "recording-error",
                    RecordingErrorEvent {
                        error_type: error_type.to_string(),
                        detail: Some(err),
                    },
                );
            }
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Prevent a slow microphone from emitting a ready event or start chime
        // after the user has already requested stop.
        app.state::<Arc<AudioRecordingManager>>()
            .invalidate_recording_readiness();

        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        set_tray_state(app, TrayIconState::Transcribing);
        // Stop should give immediate visual feedback. Live streaming can keep
        // the larger panel, but it still switches from listening to a working
        // spinner while the stream finalizes. Non-streaming paths use the
        // compact transcribing pill (None no-ops in show_*).
        let style = get_settings(app).overlay_style;
        // Capture this before finalizing the stream so every later working state
        // targets the same overlay that was shown for this transcription.
        let use_streaming_overlay = should_use_streaming_overlay(style, tm.is_streaming());
        if use_streaming_overlay {
            tm.emit_stream_working(StreamWorkKind::Transcribing);
        } else {
            show_transcribing_overlay(app);
        }

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task
        let post_process = self.post_process;
        let cancel_generation = rm.cancel_generation();
        let started_in = crate::context::take_recording_context();

        tauri::async_runtime::spawn(async move {
            let _guard = FinishGuard(ah.clone(), Arc::clone(&tm));
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            if let Some(samples) = rm.stop_recording(&binding_id, cancel_generation) {
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    samples.len()
                );

                if rm.was_cancelled_since(cancel_generation) {
                    debug!("Transcription operation cancelled after recording stop");
                    tm.cancel_stream();
                    utils::hide_recording_overlay(&ah);
                    set_tray_state(&ah, TrayIconState::Idle);
                    return;
                }

                if samples.is_empty() {
                    debug!("Recording produced no audio samples; skipping persistence");
                    // Tear down any streaming worker so its channel doesn't leak
                    // and block the next start_stream.
                    tm.cancel_stream();
                    utils::hide_recording_overlay(&ah);
                    set_tray_state(&ah, TrayIconState::Idle);
                } else {
                    // Save WAV concurrently with transcription
                    let sample_count = samples.len();
                    let file_name = format!("handy-{}.wav", chrono::Utc::now().timestamp());
                    let wav_path = hm.recordings_dir().join(&file_name);
                    let wav_path_for_verify = wav_path.clone();
                    let samples_for_wav = samples.clone();
                    let wav_handle = tauri::async_runtime::spawn_blocking(move || {
                        crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                    });

                    // Transcribe concurrently with WAV save. If a live stream was
                    // running, finalize it and use its text (all audio was already
                    // fed to the stream); otherwise batch-transcribe the samples.
                    let transcription_time = Instant::now();
                    let transcription_result = match tm.finalize_stream() {
                        // A finalized stream with usable text wins. An empty result
                        // (no active stream, produced nothing, or a finalize error
                        // after the engine was returned) falls back to a full batch
                        // transcription of the same audio. A finalize timeout is
                        // surfaced instead — the worker may still hold the engine,
                        // so a batch fallback would contend with it.
                        Ok(Some(text)) if !text.trim().is_empty() => Ok(text),
                        Ok(_) => tm.transcribe(samples),
                        Err(err) => Err(err),
                    };

                    // Await WAV save and verify
                    let wav_saved = match wav_handle.await {
                        Ok(Ok(())) => {
                            match crate::audio_toolkit::verify_wav_file(
                                &wav_path_for_verify,
                                sample_count,
                            ) {
                                Ok(()) => true,
                                Err(e) => {
                                    error!("WAV verification failed: {}", e);
                                    false
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Failed to save WAV file: {}", e);
                            false
                        }
                        Err(e) => {
                            error!("WAV save task panicked: {}", e);
                            false
                        }
                    };

                    if rm.was_cancelled_since(cancel_generation) {
                        debug!("Transcription operation cancelled before output handling");
                        utils::hide_recording_overlay(&ah);
                        set_tray_state(&ah, TrayIconState::Idle);
                        return;
                    }

                    match transcription_result {
                        Ok(transcription) => {
                            debug!(
                                "Transcription completed in {:?}: '{}'",
                                transcription_time.elapsed(),
                                utils::redact_text(&transcription)
                            );

                            if post_process {
                                if use_streaming_overlay {
                                    tm.emit_stream_working(StreamWorkKind::Polishing);
                                } else {
                                    show_processing_overlay(&ah);
                                }
                            }
                            let Some(processed) = complete_unless_cancelled(
                                process_transcription_output_in(
                                    &ah,
                                    &transcription,
                                    post_process,
                                    started_in.as_ref(),
                                ),
                                || rm.was_cancelled_since(cancel_generation),
                            )
                            .await
                            else {
                                debug!("Transcription operation cancelled during output handling");
                                utils::hide_recording_overlay(&ah);
                                set_tray_state(&ah, TrayIconState::Idle);
                                return;
                            };

                            if rm.was_cancelled_since(cancel_generation) {
                                debug!("Transcription operation cancelled before paste");
                                utils::hide_recording_overlay(&ah);
                                set_tray_state(&ah, TrayIconState::Idle);
                                return;
                            }

                            // Save to history if WAV was saved
                            if wav_saved {
                                if let Err(err) = hm.save_entry(
                                    file_name,
                                    transcription,
                                    post_process,
                                    processed.post_processed_text.clone(),
                                    processed.post_process_prompt.clone(),
                                ) {
                                    error!("Failed to save history entry: {}", err);
                                }
                            }

                            if processed.final_text.is_empty() {
                                utils::hide_recording_overlay(&ah);
                                set_tray_state(&ah, TrayIconState::Idle);
                            } else {
                                let ah_clone = ah.clone();
                                let paste_time = Instant::now();
                                let final_text = processed.final_text;
                                let rm_for_paste = Arc::clone(&rm);
                                let started_in_for_paste = started_in.clone();
                                ah.run_on_main_thread(move || {
                                    if rm_for_paste.was_cancelled_since(cancel_generation) {
                                        debug!("Transcription operation cancelled before paste");
                                        utils::hide_recording_overlay(&ah_clone);
                                        set_tray_state(&ah_clone, TrayIconState::Idle);
                                        return;
                                    }

                                    // The user may have moved to another window while the text was being prepared.
                                    if get_settings(&ah_clone).paste_focus_guard {
                                        let now = crate::context::foreground();
                                        if crate::context::focus_moved(
                                            started_in_for_paste.as_ref(),
                                            now.as_ref(),
                                        ) {
                                            use tauri_plugin_clipboard_manager::ClipboardExt;
                                            let _ = ah_clone.clipboard().write_text(final_text.clone());
                                            utils::hide_recording_overlay(&ah_clone);
                                            set_tray_state(&ah_clone, TrayIconState::Idle);
                                            crate::learn::announce_with(
                                                &ah_clone,
                                                "paste-moved",
                                                "The window changed, so the dictation is on the clipboard".to_string(),
                                                format!(
                                                    "You started speaking in {}; when the text was ready the active window was {}. It was not pasted, to keep it out of the wrong window. It is on the clipboard (and in History):\n\n{}",
                                                    crate::context::describe(started_in_for_paste.as_ref()),
                                                    crate::context::describe(now.as_ref()),
                                                    final_text
                                                ),
                                            );
                                            return;
                                        }
                                    }

                                    match utils::paste(final_text.clone(), ah_clone.clone()) {
                                        Ok(()) => {
                                            crate::context::note_paste(&ah_clone, &final_text);
                                            debug!(
                                                "Text pasted successfully in {:?}",
                                                paste_time.elapsed()
                                            )
                                        }
                                        Err(e) => {
                                            error!("Failed to paste transcription: {}", e);
                                            let _ = ah_clone.emit("paste-error", ());
                                        }
                                    }
                                    utils::hide_recording_overlay(&ah_clone);
                                    set_tray_state(&ah_clone, TrayIconState::Idle);
                                })
                                .unwrap_or_else(|e| {
                                    error!("Failed to run paste on main thread: {:?}", e);
                                    utils::hide_recording_overlay(&ah);
                                    set_tray_state(&ah, TrayIconState::Idle);
                                });
                            }
                        }
                        Err(err) => {
                            if rm.was_cancelled_since(cancel_generation) {
                                debug!(
                                    "Transcription operation cancelled after transcription error"
                                );
                                utils::hide_recording_overlay(&ah);
                                set_tray_state(&ah, TrayIconState::Idle);
                                return;
                            }

                            error!("Transcription failed: {}", err);
                            // Surface the failure to the UI (toast). The full
                            // message is also in handy.log via the line above.
                            let _ = ah.emit("transcription-error", err.to_string());
                            // Save entry with empty text so user can retry
                            if wav_saved {
                                if let Err(save_err) = hm.save_entry(
                                    file_name,
                                    String::new(),
                                    post_process,
                                    None,
                                    None,
                                ) {
                                    error!("Failed to save failed history entry: {}", save_err);
                                }
                            }
                            utils::hide_recording_overlay(&ah);
                            set_tray_state(&ah, TrayIconState::Idle);
                        }
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                // Tear down any streaming worker so its channel doesn't leak.
                tm.cancel_stream();
                utils::hide_recording_overlay(&ah);
                set_tray_state(&ah, TrayIconState::Idle);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Switch Action: swaps the language with the alternate language. (Prompts are chosen with the
// numbered prompt picker, `picker.rs`.)
enum SwitchKind {
    SwapLanguage,
}

struct SwitchAction {
    kind: SwitchKind,
}

/// Returns the entry after `current` in `items`, wrapping around. Starts at the first
/// entry when `current` is not in the list.
pub(crate) fn next_in_cycle<'a>(items: &'a [String], current: Option<&str>) -> Option<&'a String> {
    if items.is_empty() {
        return None;
    }
    let idx = current
        .and_then(|c| items.iter().position(|i| i == c))
        .map_or(0, |i| (i + 1) % items.len());
    items.get(idx)
}

/// Saves `settings`, tells the frontend, and shows the overlay notice for a setting
/// that a shortcut or CLI flag just changed.
pub(crate) fn announce_setting_change(
    app: &AppHandle,
    settings: AppSettings,
    setting: &str,
    value: &str,
    kind: &str,
    shown: &str,
) {
    log::info!("Switch: {} -> {}", setting, value);
    crate::settings::write_settings(app, settings);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "setting": setting, "value": value }),
    );
    crate::overlay::show_notice_overlay(app, kind, shown);
    // The dictation language and the speech model can be linked (Settings: "Model per language").
    if setting == "selected_language" {
        crate::model_switch::follow_language(app, value);
    }
}

/// Selects a post-processing prompt by id (the `--set-prompt` CLI flag).
pub(crate) fn set_prompt_by_id(app: &AppHandle, id: &str) {
    let mut settings = get_settings(app);
    let Some(name) = settings
        .post_process_prompts
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
    else {
        log::warn!("--set-prompt: no prompt with id '{}'", id);
        return;
    };
    settings.post_process_selected_prompt_id = Some(id.to_string());
    announce_setting_change(
        app,
        settings,
        "post_process_selected_prompt_id",
        id,
        "prompt",
        &name,
    );
}

/// Language codes are short ASCII (`no`, `en`, `zh-TW`, `auto`); anything else is rejected.
fn is_valid_language_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 16
        && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Sets the dictation language (the `--set-language` CLI flag).
pub(crate) fn set_language_code(app: &AppHandle, code: &str) {
    let code = code.trim();
    if !is_valid_language_code(code) {
        log::warn!("--set-language: invalid language code '{}'", code);
        return;
    }
    let mut settings = get_settings(app);
    settings.selected_language = code.to_string();
    announce_setting_change(app, settings, "selected_language", code, "language", code);
}

impl ShortcutAction for SwitchAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let mut settings = get_settings(app);
        // (setting name, new value, overlay notice kind, text shown in the notice)
        let changed: Option<(&str, String, &str, String)> = match self.kind {
            SwitchKind::SwapLanguage => {
                std::mem::swap(
                    &mut settings.selected_language,
                    &mut settings.alternate_language,
                );
                let lang = settings.selected_language.clone();
                Some(("selected_language", lang.clone(), "language", lang))
            }
        };
        match changed {
            Some((setting, value, kind, shown)) => {
                announce_setting_change(app, settings, setting, &value, kind, &shown);
            }
            None => log::warn!("Switch shortcut: nothing to switch"),
        }
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})", // Changed "Pressed" to "Started" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})", // Changed "Released" to "Stopped" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction {
            post_process: false,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "transcribe_with_post_process".to_string(),
        Arc::new(TranscribeAction { post_process: true }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "swap_language".to_string(),
        Arc::new(SwitchAction {
            kind: SwitchKind::SwapLanguage,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "prompt_picker".to_string(),
        Arc::new(crate::picker::PickerToggleAction {
            mode: crate::picker::Mode::Prompts,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "model_picker".to_string(),
        Arc::new(crate::picker::PickerToggleAction {
            mode: crate::picker::Mode::Models,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "reformat_selection".to_string(),
        Arc::new(crate::extras::ReformatAction) as Arc<dyn ShortcutAction>,
    );
    for n in 1..=crate::picker::MAX_ITEMS {
        map.insert(
            format!("picker_{n}"),
            Arc::new(crate::picker::PickerChoiceAction { number: n }) as Arc<dyn ShortcutAction>,
        );
    }
    map.insert(
        "picker_close".to_string(),
        Arc::new(crate::picker::PickerCloseAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "learn_correction".to_string(),
        Arc::new(crate::learn::LearnAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "paste_last".to_string(),
        Arc::new(crate::extras::PasteLastAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "rerun_next_prompt".to_string(),
        Arc::new(crate::extras::RerunAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});

#[cfg(test)]
mod tests {
    use super::{
        complete_unless_cancelled, extract_vocab_commands, is_blank_transcription, next_in_cycle,
        should_use_streaming_overlay, strip_think_block,
    };
    use crate::settings::OverlayStyle;
    use std::future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn language_codes_are_validated() {
        for ok in ["no", "en", "auto", "zh-TW"] {
            assert!(super::is_valid_language_code(ok), "{ok}");
        }
        for bad in ["", "n o", "no;rm", "waytoolongcodeforalanguage", "æø"] {
            assert!(!super::is_valid_language_code(bad), "{bad}");
        }
    }

    #[test]
    fn cycle_steps_forward_and_wraps() {
        let items: Vec<String> = ["en", "no", "sv"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            next_in_cycle(&items, Some("en")).map(String::as_str),
            Some("no")
        );
        assert_eq!(
            next_in_cycle(&items, Some("sv")).map(String::as_str),
            Some("en")
        );
        // Current value not in the list (or unset): start at the first entry.
        assert_eq!(
            next_in_cycle(&items, Some("auto")).map(String::as_str),
            Some("en")
        );
        assert_eq!(next_in_cycle(&items, None).map(String::as_str), Some("en"));
        assert_eq!(next_in_cycle(&[], Some("en")), None);
    }

    #[test]
    fn vocab_tag_is_stripped_and_word_collected() {
        let (text, words) =
            extract_vocab_commands("We talked about the DYST system.\n[[vocab: DYST]]");
        assert_eq!(text, "We talked about the DYST system.");
        assert_eq!(words, vec!["DYST".to_string()]);
    }

    #[test]
    fn vocab_only_dictation_leaves_no_text() {
        let (text, words) = extract_vocab_commands("[[vocab: Kubernetes]]");
        assert_eq!(text, "");
        assert_eq!(words, vec!["Kubernetes".to_string()]);
    }

    #[test]
    fn vocab_words_are_validated_and_deduplicated() {
        let long = "x".repeat(65);
        let input = format!(
            "Hi [[vocab: a]] [[vocab: A]] [[vocab: ]] [[vocab: {long}]] [[vocab: bad [x]]] [[vocab: \"quoted\"]]"
        );
        let (_, words) = extract_vocab_commands(&input);
        assert_eq!(words, vec!["a".to_string(), "quoted".to_string()]);
    }

    #[test]
    fn vocab_commands_are_capped() {
        let input = (0..9)
            .map(|i| format!("[[vocab: w{i}]]"))
            .collect::<String>();
        let (_, words) = extract_vocab_commands(&input);
        assert_eq!(words.len(), 5);
    }

    #[test]
    fn text_without_tags_is_unchanged() {
        let (text, words) = extract_vocab_commands("Nothing special [here] or [[else]].");
        assert_eq!(text, "Nothing special [here] or [[else]].");
        assert!(words.is_empty());
    }

    #[test]
    fn blank_transcription_is_detected() {
        assert!(is_blank_transcription(""));
        assert!(is_blank_transcription("   "));
        assert!(is_blank_transcription("\t\n  \r\n"));
    }

    #[test]
    fn non_blank_transcription_is_kept() {
        assert!(!is_blank_transcription("hello"));
        assert!(!is_blank_transcription("  hello  "));
    }

    #[test]
    fn completed_operation_returns_its_output() {
        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::ready("done"),
            || false,
        ));

        assert_eq!(result, Some("done"));
    }

    #[test]
    fn pending_operation_stops_after_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_for_thread = Arc::clone(&cancelled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            cancelled_for_thread.store(true, Ordering::Release);
        });

        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::pending::<()>(),
            || cancelled.load(Ordering::Acquire),
        ));

        cancel_thread.join().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn leading_think_block_is_stripped() {
        assert_eq!(
            strip_think_block("<think>pondering...</think>Cleaned text."),
            "Cleaned text."
        );
        assert_eq!(
            strip_think_block("  \n<think>multi\nline</think>\n  Cleaned text."),
            "Cleaned text."
        );
    }

    #[test]
    fn content_without_think_block_is_unchanged() {
        assert_eq!(strip_think_block("Cleaned text."), "Cleaned text.");
        assert_eq!(
            strip_think_block("Mentions <think> mid-sentence."),
            "Mentions <think> mid-sentence."
        );
        // Unclosed block: leave untouched rather than guess
        assert_eq!(
            strip_think_block("<think>never closed"),
            "<think>never closed"
        );
    }

    #[test]
    fn live_overlay_uses_streaming_states_only_for_streaming_models() {
        assert!(should_use_streaming_overlay(OverlayStyle::Live, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::Live, false));
        assert!(!should_use_streaming_overlay(OverlayStyle::Minimal, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::None, true));
    }
}

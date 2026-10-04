//! Meeting minutes from audio files: a port of the standalone `meeting-transcriber` script into Handy.
//!
//! One or more audio files (parts of the same session) are decoded, resampled to 16 kHz mono, cut into chunks and
//! transcribed either locally with the speech model Handy has loaded (private, the default) or with a cloud model
//! (Azure OpenAI or any OpenAI-compatible endpoint, e.g. `gpt-4o-transcribe`). The transcript is then turned into minutes
//! by the post-processing model, given a short title, and both are saved (`<folder>/<time>-<title>.md` and
//! `...-transcript.txt`) and the minutes opened. Progress goes to the Meetings page and the result to the Activity page.

use crate::audio_toolkit::audio::FrameResampler;
use crate::settings::{get_settings, write_settings, AppSettings, MeetingSettings};
use log::{info, warn};
use serde::Serialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const TARGET_HZ: u32 = 16_000;
/// Cloud chunks: ten minutes of 16 kHz mono WAV is 19 MB, under the 25 MB limit of the transcription APIs.
const CLOUD_CHUNK_SECS: usize = 10 * 60;
/// Local chunks: shorter, so one slow chunk does not hold everything and memory stays small.
const LOCAL_CHUNK_SECS: usize = 5 * 60;
const MAX_FILES: usize = 20;
const CLOUD_ATTEMPTS: u32 = 3;

const MINUTES_EN: &str = include_str!("../../fork/prompts/meeting_minutes_en.md");
const MINUTES_NO: &str = include_str!("../../fork/prompts/meeting_minutes_no.md");

static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone, specta::Type)]
pub struct MeetingProgress {
    /// decoding, transcribing, summarizing, saving, done, failed, cancelled
    pub stage: String,
    pub done: u32,
    pub total: u32,
    pub message: String,
    /// The minutes file once it exists.
    pub path: Option<String>,
}

fn emit(
    app: &AppHandle,
    stage: &str,
    done: u32,
    total: u32,
    message: impl Into<String>,
    path: Option<String>,
) {
    let _ = app.emit(
        "meeting-progress",
        MeetingProgress {
            stage: stage.to_string(),
            done,
            total,
            message: message.into(),
            path,
        },
    );
}

// ---- audio ---------------------------------------------------------------------------------------------------------

/// Decodes an audio file (MP3, M4A/AAC, WAV, FLAC, OGG/Vorbis) to mono 16 kHz samples.
pub fn decode_to_16k(path: &Path) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("{} is not an audio format Handy can read ({e}). Supported: MP3, M4A/AAC, WAV, FLAC, OGG.", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("{} has no audio track.", path.display()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Cannot decode {}: {e}", path.display()))?;

    let mut mono: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("Error reading {}: {e}", path.display())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                rate = spec.rate;
                let channels = spec.channels.count().max(1);
                let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buffer.copy_interleaved_ref(decoded);
                mono.extend(downmix(buffer.samples(), channels));
            }
            Err(Error::DecodeError(_)) => continue, // a damaged frame: skip it
            Err(e) => return Err(format!("Error decoding {}: {e}", path.display())),
        }
    }
    if mono.is_empty() || rate == 0 {
        return Err(format!("{} contains no audio.", path.display()));
    }
    Ok(resample_to_16k(&mono, rate))
}

/// Interleaved samples to mono (the average of the channels).
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn resample_to_16k(samples: &[f32], in_hz: u32) -> Vec<f32> {
    if in_hz == TARGET_HZ {
        return samples.to_vec();
    }
    let mut resampler = FrameResampler::new(
        in_hz as usize,
        TARGET_HZ as usize,
        Duration::from_millis(30),
    );
    let mut out = Vec::with_capacity(samples.len() * TARGET_HZ as usize / in_hz as usize + 1024);
    resampler.push(samples, |frame| out.extend_from_slice(frame));
    resampler.finish(|frame| out.extend_from_slice(frame));
    out
}

/// Cuts samples into pieces of `secs` seconds (the last one shorter).
pub fn chunk(samples: &[f32], secs: usize) -> Vec<&[f32]> {
    samples.chunks((secs * TARGET_HZ as usize).max(1)).collect()
}

/// 16-bit mono 16 kHz WAV file bytes.
pub fn wav_bytes(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_HZ,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::with_capacity(samples.len() * 2 + 64));
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for s in samples {
            writer
                .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

// ---- cloud transcription ------------------------------------------------------------------------------------------

/// The request URL and whether it is an Azure OpenAI deployment (which authenticates with an `api-key` header and puts the
/// model in the path) or a plain OpenAI-compatible endpoint (a bearer token and a `model` form field).
pub fn transcription_url(cfg: &MeetingSettings) -> (String, bool) {
    let base = cfg.endpoint.trim().trim_end_matches('/');
    let azure = base.contains("azure.com")
        || base.contains(".cognitiveservices.")
        || base.contains("openai.azure");
    if azure {
        let version = if cfg.api_version.trim().is_empty() {
            "2025-03-01-preview"
        } else {
            cfg.api_version.trim()
        };
        (
            format!(
                "{base}/openai/deployments/{}/audio/transcriptions?api-version={version}",
                cfg.transcribe_model.trim()
            ),
            true,
        )
    } else if base.is_empty() {
        (
            "https://api.openai.com/v1/audio/transcriptions".to_string(),
            false,
        )
    } else {
        (format!("{base}/audio/transcriptions"), false)
    }
}

/// The transcript text out of a transcription response (`{"text": "..."}`).
pub fn parse_transcription(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        format!(
            "Unexpected response: {}",
            body.chars().take(200).collect::<String>()
        )
    })?;
    value
        .get("text")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().to_string())
        .ok_or_else(|| {
            format!(
                "The response has no text: {}",
                body.chars().take(200).collect::<String>()
            )
        })
}

async fn transcribe_cloud(
    client: &reqwest::Client,
    cfg: &MeetingSettings,
    api_key: &str,
    wav: Vec<u8>,
    language: &str,
) -> Result<String, String> {
    let (url, azure) = transcription_url(cfg);
    let mut last_error = String::new();
    for attempt in 1..=CLOUD_ATTEMPTS {
        let file = reqwest::multipart::Part::bytes(wav.clone())
            .file_name("chunk.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;
        let mut form = reqwest::multipart::Form::new().part("file", file);
        if !language.is_empty() && language != "auto" {
            form = form.text("language", language.to_string());
        }
        if !azure {
            form = form.text("model", cfg.transcribe_model.trim().to_string());
        }
        let request = client.post(&url).multipart(form);
        let request = if azure {
            request.header("api-key", api_key)
        } else {
            request.bearer_auth(api_key)
        };
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                if status.is_success() {
                    return parse_transcription(&body);
                }
                last_error = format!(
                    "HTTP {status}: {}",
                    body.chars().take(300).collect::<String>()
                );
                // Rate limits and server errors are worth another try; anything else (bad key, wrong deployment) is not.
                if !(status.as_u16() == 429 || status.is_server_error()) {
                    return Err(last_error);
                }
            }
            Err(e) => last_error = format!("Request failed: {e}"),
        }
        if attempt < CLOUD_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
    }
    Err(last_error)
}

// ---- text ---------------------------------------------------------------------------------------------------------

/// A file name from a title: no characters Windows or Unix reject, spaces become dashes, at most 80 characters.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { ' ' } else { c })
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let limited: String = collapsed.chars().take(80).collect();
    let dashed = limited.trim().replace(' ', "-");
    if dashed.is_empty() {
        "untitled".to_string()
    } else {
        dashed
    }
}

/// The prompt for the minutes: the user's own `t_meeting_minutes_<lang>` prompt if it exists in the prompt list (so it can
/// be edited there), otherwise the built-in one. `${output}` is replaced by the transcript in a single pass.
pub fn minutes_prompt(settings: &AppSettings, language: &str, transcript: &str) -> String {
    let norwegian = matches!(language.to_lowercase().as_str(), "no" | "nb" | "nn");
    let id = if norwegian {
        "t_meeting_minutes_no"
    } else {
        "t_meeting_minutes_en"
    };
    let template = settings
        .post_process_prompts
        .iter()
        .find(|p| p.id == id && !p.prompt.trim().is_empty())
        .map(|p| p.prompt.clone())
        .unwrap_or_else(|| (if norwegian { MINUTES_NO } else { MINUTES_EN }).to_string());
    fill_transcript(&template, transcript)
}

pub fn fill_transcript(template: &str, transcript: &str) -> String {
    match template.find("${output}") {
        Some(at) => format!(
            "{}{}{}",
            &template[..at],
            transcript,
            &template[at + "${output}".len()..]
        ),
        None => format!("{template}\n\n{transcript}"),
    }
}

fn title_prompt(language: &str, minutes: &str) -> String {
    if matches!(language.to_lowercase().as_str(), "no" | "nb" | "nn") {
        format!("Lag en kort (3-8 ord) tittel som beskriver innholdet. Skriv bare tittelen uten anførselstegn eller avsluttende tegn.\n\nReferat:\n{minutes}\n")
    } else {
        format!("Create a short (3-8 word) title that describes the content. Output only the title with no quotes or trailing punctuation.\n\nMinutes:\n{minutes}\n")
    }
}

fn output_dir(cfg: &MeetingSettings) -> PathBuf {
    if !cfg.output_dir.trim().is_empty() {
        return PathBuf::from(cfg.output_dir.trim());
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join("meeting_notes")
}

// ---- the job --------------------------------------------------------------------------------------------------------

/// `files` are parts of one session, in order. Returns the path of the minutes.
async fn run_job(
    app: &AppHandle,
    files: &[PathBuf],
    language: &str,
    engine: &str,
) -> Result<PathBuf, String> {
    let settings = get_settings(app);
    let cfg = settings.meeting.clone();
    let cloud = engine == "cloud";
    let api_key = settings
        .post_process_api_keys
        .get("meeting")
        .cloned()
        .unwrap_or_default();
    if cloud
        && (cfg.endpoint.trim().is_empty()
            || api_key.is_empty()
            || cfg.transcribe_model.trim().is_empty())
    {
        return Err(
            "The cloud engine needs an endpoint, a model and an API key (Meetings page).".into(),
        );
    }
    if !cloud
        && !app
            .state::<Arc<crate::managers::transcription::TranscriptionManager>>()
            .inner()
            .is_model_loaded()
    {
        app.state::<Arc<crate::managers::transcription::TranscriptionManager>>()
            .initiate_model_load();
    }

    // 1. decode every part
    let mut pieces: Vec<Vec<f32>> = Vec::new();
    let mut total_secs = 0usize;
    for (i, path) in files.iter().enumerate() {
        emit(
            app,
            "decoding",
            i as u32,
            files.len() as u32,
            format!("Reading {}", path.display()),
            None,
        );
        let path_owned = path.clone();
        let samples = tokio::task::spawn_blocking(move || decode_to_16k(&path_owned))
            .await
            .map_err(|e| e.to_string())??;
        total_secs += samples.len() / TARGET_HZ as usize;
        pieces.push(samples);
        if CANCEL.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
    }

    // 2. transcribe chunk by chunk (chunks are global across the parts, like the original script)
    let secs = if cloud {
        CLOUD_CHUNK_SECS
    } else {
        LOCAL_CHUNK_SECS
    };
    let chunks: Vec<&[f32]> = pieces.iter().flat_map(|p| chunk(p, secs)).collect();
    let total = chunks.len() as u32;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut transcripts: Vec<String> = Vec::new();
    for (i, piece) in chunks.iter().enumerate() {
        if CANCEL.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        emit(
            app,
            "transcribing",
            i as u32,
            total,
            format!(
                "Transcribing chunk {} of {} ({} s)",
                i + 1,
                total,
                piece.len() / TARGET_HZ as usize
            ),
            None,
        );
        let text = if cloud {
            transcribe_cloud(&client, &cfg, &api_key, wav_bytes(piece)?, language).await?
        } else {
            let audio = piece.to_vec();
            let handle = app.clone();
            tokio::task::spawn_blocking(move || {
                handle
                    .state::<Arc<crate::managers::transcription::TranscriptionManager>>()
                    .transcribe(audio)
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())??
        };
        transcripts.push(text.trim().to_string());
    }
    let transcript = transcripts
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if transcript.trim().is_empty() {
        return Err("The transcription came back empty.".into());
    }

    // 3. minutes and title from the post-processing model
    emit(
        app,
        "summarizing",
        total,
        total,
        "Writing the minutes",
        None,
    );
    let minutes = crate::learn::ask_text(&settings, minutes_prompt(&settings, language, &transcript))
        .await
        .ok_or("The post-processing model could not be reached, so there are no minutes. The transcript was saved.")
        .map(|m| m.trim().to_string());
    let title = match &minutes {
        Ok(m) => crate::learn::ask_text(&settings, title_prompt(language, m))
            .await
            .and_then(|t| t.lines().next().map(str::to_string)),
        Err(_) => None,
    };

    // 4. save
    emit(app, "saving", total, total, "Saving", None);
    let dir = output_dir(&cfg);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y-%m-%dT%H%M%S").to_string();
    let name = safe_filename(&title.unwrap_or_else(|| {
        if language == "no" {
            "Møte".into()
        } else {
            "Meeting".into()
        }
    }));
    let transcript_path = dir.join(format!("{stamp}-{name}-transcript.txt"));
    std::fs::write(&transcript_path, &transcript)
        .map_err(|e| format!("Cannot write {}: {e}", transcript_path.display()))?;
    match minutes {
        Ok(text) => {
            let minutes_path = dir.join(format!("{stamp}-{name}.md"));
            std::fs::write(&minutes_path, text)
                .map_err(|e| format!("Cannot write {}: {e}", minutes_path.display()))?;
            crate::activity::log(
                app,
                "meeting",
                &format!("{name} ({} min)", total_secs / 60),
                &format!(
                    "Engine: {engine}\nLanguage: {language}\nFiles: {}\nMinutes: {}\nTranscript: {}",
                    files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join("; "),
                    minutes_path.display(),
                    transcript_path.display()
                ),
            );
            Ok(minutes_path)
        }
        Err(reason) => Err(format!(
            "{reason} Transcript: {}",
            transcript_path.display()
        )),
    }
}

/// Starts a job on its own thread. Only one runs at a time.
pub fn start(
    app: &AppHandle,
    files: Vec<PathBuf>,
    language: String,
    engine: String,
) -> Result<(), String> {
    if files.is_empty() {
        return Err("No audio file was given.".into());
    }
    if files.len() > MAX_FILES {
        return Err(format!("At most {MAX_FILES} files at a time."));
    }
    if let Some(missing) = files.iter().find(|f| !f.is_file()) {
        return Err(format!("File not found: {}", missing.display()));
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("A meeting transcription is already running.".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        info!(
            "Meeting: {} file(s), language {language}, engine {engine}",
            files.len()
        );
        let result = run_job(&app, &files, &language, &engine).await;
        RUNNING.store(false, Ordering::SeqCst);
        match result {
            Ok(path) => {
                emit(&app, "done", 1, 1, "Done", Some(path.display().to_string()));
                crate::learn::announce(&app, "meeting", path.display().to_string());
                use tauri_plugin_opener::OpenerExt;
                if let Err(e) = app
                    .opener()
                    .open_path(path.display().to_string(), None::<&str>)
                {
                    warn!("Meeting: could not open the minutes: {e}");
                }
            }
            Err(reason) if reason == "cancelled" => {
                emit(&app, "cancelled", 0, 0, "Cancelled", None)
            }
            Err(reason) => {
                warn!("Meeting failed: {reason}");
                crate::activity::log(&app, "meeting-failed", "Meeting minutes failed", &reason);
                emit(&app, "failed", 0, 0, reason, None);
            }
        }
    });
    Ok(())
}

/// Splits `;`-separated paths (the command line form for several parts).
pub fn split_paths(list: &str) -> Vec<PathBuf> {
    list.split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// `handy --meeting-minutes "a.m4a;b.m4a" [--meeting-language no] [--meeting-engine cloud]`
pub fn run_cli(app: &AppHandle, list: &str, language: Option<String>, engine: Option<String>) {
    let cfg = get_settings(app).meeting;
    let language = language.unwrap_or(cfg.language);
    let engine = engine.unwrap_or(cfg.engine);
    if let Err(reason) = start(app, split_paths(list), language, engine) {
        warn!("Meeting: {reason}");
        crate::activity::log(
            app,
            "meeting-failed",
            "Meeting minutes could not start",
            &reason,
        );
    }
}

#[tauri::command]
#[specta::specta]
pub fn start_meeting(
    app: AppHandle,
    files: Vec<String>,
    language: String,
    engine: String,
) -> Result<(), String> {
    start(
        &app,
        files.into_iter().map(PathBuf::from).collect(),
        language,
        engine,
    )
}

/// Opens a saved minutes or transcript file, but only one inside the minutes folder.
#[tauri::command]
#[specta::specta]
pub fn open_meeting_file(app: AppHandle, path: String) -> Result<(), String> {
    let dir = output_dir(&get_settings(&app).meeting);
    let wanted = std::fs::canonicalize(&path).map_err(|e| format!("Cannot open {path}: {e}"))?;
    let allowed =
        std::fs::canonicalize(&dir).map_err(|e| format!("Cannot open {}: {e}", dir.display()))?;
    if !wanted.starts_with(&allowed) {
        return Err("Only files in the meeting notes folder can be opened here.".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_meeting() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
#[specta::specta]
pub fn update_meeting_settings(app: AppHandle, meeting: MeetingSettings) -> Result<(), String> {
    if !matches!(meeting.engine.as_str(), "local" | "cloud") {
        return Err("The engine must be 'local' or 'cloud'.".into());
    }
    let mut settings = get_settings(&app);
    settings.meeting = meeting;
    write_settings(&app, settings);
    Ok(())
}

/// The cloud API key is kept with the other keys, which are never written to the log.
#[tauri::command]
#[specta::specta]
pub fn set_meeting_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings
        .post_process_api_keys
        .insert("meeting".to_string(), api_key);
    write_settings(&app, settings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(endpoint: &str) -> MeetingSettings {
        MeetingSettings {
            endpoint: endpoint.into(),
            transcribe_model: "gpt-4o-transcribe".into(),
            api_version: "2025-03-01-preview".into(),
            ..MeetingSettings::default()
        }
    }

    #[test]
    fn azure_and_openai_urls() {
        let (url, azure) = transcription_url(&cfg("https://x.cognitiveservices.azure.com/"));
        assert!(azure);
        assert_eq!(
            url,
            "https://x.cognitiveservices.azure.com/openai/deployments/gpt-4o-transcribe/audio/transcriptions?api-version=2025-03-01-preview"
        );
        let (url, azure) = transcription_url(&cfg(""));
        assert!(!azure);
        assert_eq!(url, "https://api.openai.com/v1/audio/transcriptions");
        let (url, _) = transcription_url(&cfg("https://gateway.example/v1/"));
        assert_eq!(url, "https://gateway.example/v1/audio/transcriptions");
    }

    #[test]
    fn transcription_responses() {
        assert_eq!(
            parse_transcription("{\"text\": \"  hello \"}").unwrap(),
            "hello"
        );
        assert!(parse_transcription("{\"error\": 1}").is_err());
        assert!(parse_transcription("<html>").is_err());
    }

    #[test]
    fn filenames_are_safe() {
        assert_eq!(
            safe_filename("Budget: Q3/Q4 review?"),
            "Budget-Q3-Q4-review"
        );
        assert_eq!(safe_filename("  "), "untitled");
        assert_eq!(safe_filename("Møte om økonomi"), "Møte-om-økonomi");
        assert!(safe_filename(&"x".repeat(200)).chars().count() <= 80);
    }

    #[test]
    fn the_transcript_goes_in_once_and_cannot_hijack_the_prompt() {
        let out = fill_transcript("Minutes of:\n${output}\nEnd", "a ${output} b");
        assert_eq!(out, "Minutes of:\na ${output} b\nEnd");
        assert_eq!(
            fill_transcript("No placeholder", "T"),
            "No placeholder\n\nT"
        );
    }

    #[test]
    fn audio_math() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(downmix(&[0.1, 0.2], 1), vec![0.1, 0.2]);
        let samples = vec![0.0f32; TARGET_HZ as usize * 25];
        let parts = chunk(&samples, 10);
        assert_eq!(
            parts
                .iter()
                .map(|p| p.len() / TARGET_HZ as usize)
                .collect::<Vec<_>>(),
            vec![10, 10, 5]
        );
        assert_eq!(wav_bytes(&[0.0; 160]).unwrap().len(), 44 + 320);
        assert_eq!(
            split_paths("a.m4a; b.m4a;;"),
            vec![PathBuf::from("a.m4a"), PathBuf::from("b.m4a")]
        );
    }

    /// A one-shot HTTP server on localhost: reads one request completely, answers with `body`, and returns what it read.
    fn fake_server(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut data: Vec<u8> = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = stream.read(&mut buf).unwrap();
                data.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&data);
                if let Some(split) = text.find("\r\n\r\n") {
                    let length = text[..split]
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if data.len() >= split + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&data).to_string()
        });
        (base, handle)
    }

    #[test]
    fn the_cloud_request_has_the_right_shape_for_openai_and_azure() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let wav = wav_bytes(&[0.0; 1600]).unwrap();
        let client = reqwest::Client::new();

        // OpenAI-compatible: bearer token, model and language as form fields, file part
        let (base, server) = fake_server("{\"text\": \" hello there \"}");
        let text = rt
            .block_on(transcribe_cloud(
                &client,
                &cfg(&base),
                "secret-key",
                wav.clone(),
                "no",
            ))
            .unwrap();
        assert_eq!(text, "hello there");
        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /audio/transcriptions"),
            "{}",
            &request[..60]
        );
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer secret-key"));
        assert!(request.contains("name=\"model\"") && request.contains("gpt-4o-transcribe"));
        assert!(request.contains("name=\"language\"") && request.contains("\r\n\r\nno\r\n"));
        assert!(
            request.contains("name=\"file\"")
                && request.contains("filename=\"chunk.wav\"")
                && request.contains("RIFF")
        );

        // Azure: the model is in the path, the key is an api-key header, there is no model field. (The fake server is on
        // localhost, so force the Azure form by putting the marker in the path.)
        let (base, server) = fake_server("{\"text\": \"azure ok\"}");
        let azure_like = cfg(&format!("{base}/x.cognitiveservices.azure.com"));
        let (url, is_azure) = transcription_url(&azure_like);
        assert!(
            is_azure
                && url.contains(
                    "/openai/deployments/gpt-4o-transcribe/audio/transcriptions?api-version="
                )
        );
        let text = rt
            .block_on(transcribe_cloud(
                &client,
                &azure_like,
                "azure-key",
                wav.clone(),
                "",
            ))
            .unwrap();
        assert_eq!(text, "azure ok");
        let request = server.join().unwrap();
        assert!(request.to_lowercase().contains("api-key: azure-key"));
        assert!(!request.to_lowercase().contains("authorization:"));
        assert!(!request.contains("name=\"model\"") && !request.contains("name=\"language\""));
    }

    #[test]
    fn a_wav_file_is_decoded_downmixed_and_resampled() {
        let dir = std::env::temp_dir().join(format!("handy-meeting-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..48_000 * 2 {
            // two seconds of a 440 Hz tone on both channels
            let s = ((i as f32 / 48_000.0) * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            let v = (s * i16::MAX as f32) as i16;
            writer.write_sample(v).unwrap();
            writer.write_sample(v).unwrap();
        }
        writer.finalize().unwrap();
        let samples = decode_to_16k(&path).unwrap();
        let seconds = samples.len() as f32 / TARGET_HZ as f32;
        assert!((seconds - 2.0).abs() < 0.15, "{seconds}");
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.3 && peak < 0.7, "{peak}");
        assert!(decode_to_16k(&dir.join("missing.wav")).is_err());
        std::fs::write(dir.join("junk.mp3"), b"not audio at all").unwrap();
        assert!(decode_to_16k(&dir.join("junk.mp3")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

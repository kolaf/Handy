//! Switching the speech-to-text model by name, from the command line (`handy --set-model parakeet`), the model picker
//! (`handy --model-picker`) or Talon. Only downloaded models can be chosen. The switch itself is the one the settings page
//! and the tray menu use (`commands::models::switch_active_model`), so the model is loaded the same way.

use crate::commands::models::switch_active_model;
use crate::managers::model::{ModelInfo, ModelManager};
use crate::settings::get_settings;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// The downloaded models, in the catalog's order.
pub fn downloaded(app: &AppHandle) -> Vec<ModelInfo> {
    app.state::<Arc<ModelManager>>()
        .get_available_models()
        .into_iter()
        .filter(|m| m.is_downloaded)
        .collect()
}

/// Which model does `query` mean? An exact id wins; otherwise the words must occur, ignoring case, in the id or name of
/// exactly one candidate. `candidates` are (id, name) pairs of the downloaded models.
pub fn resolve_id(candidates: &[(String, String)], query: &str) -> Result<String, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err("No model name was given.".into());
    }
    if let Some((id, _)) = candidates.iter().find(|(id, _)| id.to_lowercase() == q) {
        return Ok(id.clone());
    }
    let hits: Vec<&(String, String)> = candidates
        .iter()
        .filter(|(id, name)| id.to_lowercase().contains(&q) || name.to_lowercase().contains(&q))
        .collect();
    match hits.as_slice() {
        [one] => Ok(one.0.clone()),
        [] => Err(format!(
            "No downloaded model matches '{query}'. Downloaded: {}.",
            list_names(candidates)
        )),
        many => Err(format!(
            "'{query}' matches several models ({}); be more specific.",
            many.iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn list_names(candidates: &[(String, String)]) -> String {
    if candidates.is_empty() {
        "none".to_string()
    } else {
        candidates
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Switches to the model with exactly this id and tells the user (toast and activity log). Blocks while the model loads,
/// so call it from a thread.
pub fn switch_and_report(app: &AppHandle, id: &str) {
    let models = downloaded(app);
    let Some(info) = models.iter().find(|m| m.id == id) else {
        crate::learn::announce_with(
            app,
            "model-failed",
            id.to_string(),
            format!("'{id}' is not a downloaded model."),
        );
        return;
    };
    let settings = get_settings(app);
    if settings.selected_model == id {
        crate::learn::announce_with(
            app,
            "model",
            info.name.clone(),
            "Already the active model.".into(),
        );
        return;
    }
    match switch_active_model(app, id) {
        Ok(()) => {
            let mut details = vec![format!(
                "Switched from '{}' to '{}' ({}).",
                settings.selected_model, info.name, info.id
            )];
            let language = settings.selected_language.as_str();
            if language != "auto"
                && !info.supported_languages.is_empty()
                && !info.supported_languages.iter().any(|l| l == language)
            {
                details.push(format!(
                    "Note: this model does not list '{language}' as a supported language; Handy will use the closest one it can."
                ));
            }
            crate::learn::announce_with(app, "model", info.name.clone(), details.join("\n"));
        }
        Err(err) => crate::learn::announce_with(app, "model-failed", info.name.clone(), err),
    }
}

/// `handy --set-model QUERY`
pub fn run_set(app: &AppHandle, query: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let pairs: Vec<(String, String)> = downloaded(&app)
            .into_iter()
            .map(|m| (m.id, m.name))
            .collect();
        match resolve_id(&pairs, &query) {
            Ok(id) => switch_and_report(&app, &id),
            Err(reason) => crate::learn::announce_with(&app, "model-failed", query, reason),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models() -> Vec<(String, String)> {
        vec![
            ("nb_ggml-model-q5_0".into(), "NB-Whisper medium".into()),
            ("parakeet-tdt-0.6b-v2".into(), "Parakeet V2".into()),
            ("parakeet-tdt-0.6b-v3".into(), "Parakeet V3".into()),
            ("small".into(), "Whisper Small".into()),
        ]
    }

    #[test]
    fn an_exact_id_wins_even_when_it_is_part_of_other_names() {
        assert_eq!(resolve_id(&models(), "small").unwrap(), "small");
        assert_eq!(
            resolve_id(&models(), "PARAKEET-TDT-0.6B-V3").unwrap(),
            "parakeet-tdt-0.6b-v3"
        );
    }

    #[test]
    fn words_in_the_id_or_name_find_a_single_model() {
        assert_eq!(resolve_id(&models(), "nb").unwrap(), "nb_ggml-model-q5_0");
        assert_eq!(resolve_id(&models(), "whisper small").unwrap(), "small");
        assert_eq!(
            resolve_id(&models(), " v3 ").unwrap(),
            "parakeet-tdt-0.6b-v3"
        );
    }

    #[test]
    fn unknown_empty_or_ambiguous_names_explain_themselves() {
        let err = resolve_id(&models(), "gigaam").unwrap_err();
        assert!(
            err.contains("No downloaded model matches") && err.contains("Parakeet V2"),
            "{err}"
        );
        let err = resolve_id(&models(), "parakeet").unwrap_err();
        assert!(
            err.contains("several") && err.contains("Parakeet V3"),
            "{err}"
        );
        assert!(resolve_id(&models(), "  ").is_err());
        assert!(resolve_id(&[], "x").unwrap_err().contains("none"));
    }
}

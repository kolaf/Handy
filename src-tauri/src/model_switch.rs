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

/// What to do when the dictation language becomes `language`.
#[derive(Debug, PartialEq)]
pub enum LanguagePlan {
    /// No rule for the language, or the model is already the right one.
    Nothing,
    /// Switch to this downloaded model.
    Switch(String),
    /// The rule names a model that is not downloaded.
    Missing(String),
}

pub fn plan_for_language(
    rules: &[crate::settings::LanguageModel],
    language: &str,
    current_model: &str,
    downloaded_ids: &[String],
) -> LanguagePlan {
    let Some(rule) = rules
        .iter()
        .find(|r| r.language.eq_ignore_ascii_case(language.trim()))
    else {
        return LanguagePlan::Nothing;
    };
    if rule.model_id == current_model {
        LanguagePlan::Nothing
    } else if downloaded_ids.contains(&rule.model_id) {
        LanguagePlan::Switch(rule.model_id.clone())
    } else {
        LanguagePlan::Missing(rule.model_id.clone())
    }
}

/// Called after the dictation language changed: if the link is on and a rule names a model for it, switch to that model.
/// Runs the switch on its own thread (loading a model takes a moment).
pub fn follow_language(app: &AppHandle, language: &str) {
    let settings = get_settings(app);
    if !settings.language_models_enabled || settings.language_models.is_empty() {
        return;
    }
    let ids: Vec<String> = downloaded(app).into_iter().map(|m| m.id).collect();
    match plan_for_language(
        &settings.language_models,
        language,
        &settings.selected_model,
        &ids,
    ) {
        LanguagePlan::Nothing => {}
        LanguagePlan::Switch(id) => {
            let app = app.clone();
            std::thread::spawn(move || switch_and_report(&app, &id));
        }
        LanguagePlan::Missing(id) => crate::learn::announce_with(
            app,
            "model-failed",
            id.clone(),
            format!("The model for language '{language}' is '{id}', but it is not downloaded."),
        ),
    }
}

fn valid_rules(
    rules: &[crate::settings::LanguageModel],
    known_ids: &[String],
) -> Result<(), String> {
    if rules.len() > 30 {
        return Err("At most 30 language rules are allowed".into());
    }
    for (i, rule) in rules.iter().enumerate() {
        let lang = rule.language.trim();
        if lang.is_empty() || lang.chars().count() > 12 || lang.chars().any(|c| c.is_control()) {
            return Err(format!("Invalid language '{}'", rule.language));
        }
        if !known_ids.contains(&rule.model_id) {
            return Err(format!("Unknown model '{}'", rule.model_id));
        }
        if rules[..i]
            .iter()
            .any(|o| o.language.eq_ignore_ascii_case(lang))
        {
            return Err(format!("Language '{lang}' appears twice"));
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_language_models(
    app: AppHandle,
    rules: Vec<crate::settings::LanguageModel>,
) -> Result<(), String> {
    let known: Vec<String> = app
        .state::<Arc<ModelManager>>()
        .get_available_models()
        .into_iter()
        .map(|m| m.id)
        .collect();
    valid_rules(&rules, &known)?;
    let mut settings = get_settings(&app);
    settings.language_models = rules;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_language_models_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.language_models_enabled = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
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

/// Picks a post-processing provider by `local`, `cloud` (the custom provider, where the LiteLLM address is entered), or by
/// (part of) its id or label.
pub fn resolve_provider(
    providers: &[crate::settings::PostProcessProvider],
    query: &str,
) -> Result<String, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err("No language model was named. Say local or cloud.".into());
    }
    let wanted = match q.as_str() {
        "cloud" | "remote" => crate::settings::CLOUD_PROVIDER_ID,
        other => other,
    };
    if let Some(p) = providers.iter().find(|p| p.id.to_lowercase() == wanted) {
        return Ok(p.id.clone());
    }
    let matches: Vec<_> = providers
        .iter()
        .filter(|p| p.label.to_lowercase().contains(wanted) || p.id.to_lowercase().contains(wanted))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => Err(format!(
            "No language model provider matches '{query}'. Available: {}.",
            providers
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        many => Err(format!(
            "'{query}' matches several providers: {}.",
            many.iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// `handy --set-llm local|cloud|NAME`: the language model for post-processing, without opening the settings page.
pub fn run_set_llm(app: &AppHandle, query: String) {
    let mut settings = get_settings(app);
    match resolve_provider(&settings.post_process_providers, &query) {
        Ok(id) => {
            let provider = settings
                .post_process_providers
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .expect("resolved provider exists");
            let model = settings
                .post_process_models
                .get(&id)
                .cloned()
                .unwrap_or_default();
            let mut details = format!("{} at {}", provider.label, provider.base_url);
            if model.trim().is_empty() {
                details.push_str("\nNo model name is set for it yet (Post-Processing page): post-processing will be skipped until one is.");
            }
            if settings.post_process_provider_id == id {
                details = format!("Already in use. {details}");
            } else {
                settings.post_process_provider_id = id.clone();
                crate::settings::write_settings(app, settings);
                use tauri::Emitter;
                let _ = app.emit(
                    "settings-changed",
                    serde_json::json!({ "setting": "post_process_provider_id", "value": id }),
                );
            }
            crate::learn::announce_with(app, "llm", provider.label, details);
        }
        Err(reason) => crate::learn::announce_with(app, "llm-failed", query, reason),
    }
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
    fn a_language_rule_picks_the_downloaded_model_unless_it_is_already_active() {
        use crate::settings::LanguageModel;
        let rules = vec![
            LanguageModel {
                language: "no".into(),
                model_id: "nb".into(),
            },
            LanguageModel {
                language: "en".into(),
                model_id: "parakeet".into(),
            },
        ];
        let have = vec!["nb".to_string(), "parakeet".to_string()];
        assert_eq!(
            plan_for_language(&rules, "en", "nb", &have),
            LanguagePlan::Switch("parakeet".into())
        );
        assert_eq!(
            plan_for_language(&rules, "EN", "parakeet", &have),
            LanguagePlan::Nothing
        );
        assert_eq!(
            plan_for_language(&rules, "sv", "nb", &have),
            LanguagePlan::Nothing
        );
        assert_eq!(
            plan_for_language(&rules, "en", "nb", &["nb".to_string()]),
            LanguagePlan::Missing("parakeet".into())
        );
        assert_eq!(
            plan_for_language(&[], "en", "nb", &have),
            LanguagePlan::Nothing
        );
    }

    #[test]
    fn language_rules_are_validated() {
        use crate::settings::LanguageModel;
        let known = vec!["nb".to_string(), "parakeet".to_string()];
        let rule = |l: &str, m: &str| LanguageModel {
            language: l.into(),
            model_id: m.into(),
        };
        assert!(valid_rules(&[rule("no", "nb"), rule("en", "parakeet")], &known).is_ok());
        assert!(valid_rules(&[rule("", "nb")], &known).is_err());
        assert!(valid_rules(&[rule("no", "ghost")], &known).is_err());
        assert!(valid_rules(&[rule("no", "nb"), rule("NO", "parakeet")], &known).is_err());
        assert!(valid_rules(&[rule("a\tb", "nb")], &known).is_err());
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

    fn providers() -> Vec<crate::settings::PostProcessProvider> {
        [
            "openai:OpenAI",
            "local:Local (llama-server)",
            "custom:Custom",
        ]
        .iter()
        .map(|p| {
            let (id, label) = p.split_once(':').unwrap();
            crate::settings::PostProcessProvider {
                id: id.into(),
                label: label.into(),
                base_url: String::new(),
                allow_base_url_edit: false,
                models_endpoint: None,
                supports_structured_output: false,
            }
        })
        .collect()
    }

    #[test]
    fn local_and_cloud_pick_the_right_provider() {
        assert_eq!(resolve_provider(&providers(), "local").unwrap(), "local");
        assert_eq!(resolve_provider(&providers(), "Cloud").unwrap(), "custom");
        assert_eq!(resolve_provider(&providers(), "open").unwrap(), "openai");
        assert!(resolve_provider(&providers(), "").is_err());
        assert!(resolve_provider(&providers(), "banana")
            .unwrap_err()
            .contains("Available"));
    }
}

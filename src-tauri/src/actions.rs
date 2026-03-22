use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{self, show_recording_overlay, show_structuring_overlay, show_transcribing_overlay};
use crate::ManagedToggleState;
use crate::{anamedi_client, audio_toolkit::save_wav_file};
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error};
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Manager;

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction;

#[derive(Clone, Debug)]
pub struct ProcessedTranscription {
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
}

async fn maybe_post_process_transcription(
    settings: &AppSettings,
    transcription: &str,
    audio_samples: &[f32],
) -> Option<String> {
    if !settings.post_process_enabled {
        return None;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return None;
        }
    };

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

    // When using Anamedi as provider, we call the dedicated audio endpoint instead of
    // the text-based OpenAI-compatible chat completion API.
    if provider.id == "anamedi" {
        return maybe_post_process_with_anamedi(settings, transcription, audio_samples, &prompt)
            .await;
    }

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

    if prompt.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return None;
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    // Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Send the chat completion request
    match crate::llm_client::send_chat_completion(&provider, api_key, &model, processed_prompt)
        .await
    {
        Ok(Some(content)) => {
            // Strip invisible Unicode characters that some LLMs (e.g., Qwen) may insert
            let content = content
                .replace('\u{200B}', "") // Zero-Width Space
                .replace('\u{200C}', "") // Zero-Width Non-Joiner
                .replace('\u{200D}', "") // Zero-Width Joiner
                .replace('\u{FEFF}', ""); // Byte Order Mark / Zero-Width No-Break Space
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

/// Try to map the app's selected_language to an ISO 639-3 code for Anamedi.
fn map_language_to_iso_639_3(selected_language: &str) -> Option<Cow<'static, str>> {
    let normalized = selected_language.to_lowercase();
    match normalized.as_str() {
        "auto" | "" => None,
        "en" | "en-us" | "en-gb" => Some(Cow::Borrowed("eng")),
        "de" | "de-de" => Some(Cow::Borrowed("deu")),
        "fr" | "fr-fr" => Some(Cow::Borrowed("fra")),
        "it" | "it-it" => Some(Cow::Borrowed("ita")),
        "es" | "es-es" => Some(Cow::Borrowed("spa")),
        // Fallback: let Anamedi auto-detect
        _ => None,
    }
}

/// Perform post-processing using Anamedi's /api/transcribe-custom-structure endpoint.
///
/// This keeps dictation/transcription local while optionally offloading the
/// summarization/structuring step to Anamedi when explicitly configured.
async fn maybe_post_process_with_anamedi(
    settings: &AppSettings,
    _transcription: &str,
    audio_samples: &[f32],
    prompt_text: &str,
) -> Option<String> {
    // API key is stored in the generic post_process_api_keys map under the Anamedi provider id.
    let api_key = match settings.post_process_api_keys.get("anamedi") {
        Some(key) if !key.trim().is_empty() => key.trim().to_string(),
        _ => {
            debug!("Anamedi post-processing skipped because API key is missing");
            return None;
        }
    };

    // Derive schema / instructions from the selected prompt:
    // - If the prompt is valid JSON, treat it as the schema and omit instructions.
    // - Otherwise, use a built-in SOAP-style schema and pass the prompt as instructions.
    let (schema, instructions): (String, Option<String>) =
        if serde_json::from_str::<serde_json::Value>(prompt_text).is_ok() {
            (prompt_text.to_string(), None)
        } else {
            let default_schema = r#"{
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Structured medical summary of the encounter in free text"
    },
    "keywords": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Important clinical keywords and concepts"
    },
    "followUp": {
      "type": "string",
      "description": "Recommended follow-up actions or next steps"
    }
  },
  "required": ["summary"]
}"#;
            (default_schema.to_string(), Some(prompt_text.to_string()))
        };

    // Map language if possible, otherwise let Anamedi auto-detect.
    let language_code = map_language_to_iso_639_3(&settings.selected_language);

    // Write a temporary WAV file for Anamedi from the in-memory samples.
    let timestamp = chrono::Utc::now().timestamp_millis();
    let temp_dir = env::temp_dir();
    let temp_path: PathBuf = temp_dir.join(format!("anamedi-{}.wav", timestamp));

    if let Err(e) = save_wav_file(&temp_path, audio_samples).await {
        error!(
            "Failed to write temporary WAV file for Anamedi at {:?}: {}",
            temp_path, e
        );
        return None;
    }

    let result = anamedi_client::transcribe_custom_structure_with_file(
        &api_key,
        &temp_path,
        &schema,
        instructions.as_deref(),
        None,
        language_code.as_deref(),
    )
    .await;

    // Best-effort cleanup of the temporary audio file.
    if let Err(e) = std::fs::remove_file(&temp_path) {
        debug!(
            "Failed to remove temporary Anamedi WAV file {:?}: {}",
            temp_path, e
        );
    }

    match result {
        Ok(response) => {
            debug!(
                "Anamedi post-processing succeeded. Transcript length: {}, structuredData keys: {}",
                response.transcript.len(),
                match response.structured_data.as_object() {
                    Some(obj) => obj.keys().count(),
                    None => 0,
                }
            );

            // Prefer a top-level "summary" field if present, otherwise fall back to pretty JSON.
            if let Some(summary) = response
                .structured_data
                .get("summary")
                .and_then(|v| v.as_str())
            {
                Some(summary.to_string())
            } else {
                match serde_json::to_string_pretty(&response.structured_data) {
                    Ok(text) => Some(text),
                    Err(e) => {
                        error!("Failed to serialize Anamedi structuredData to JSON: {}", e);
                        None
                    }
                }
            }
        }
        Err(e) => {
            error!(
                "Anamedi post-processing failed: {}. Falling back to local transcription only.",
                e
            );
            None
        }
    }
}

async fn maybe_convert_chinese_variant(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    // Check if language is set to Simplified or Traditional Chinese
    let is_simplified = settings.selected_language == "zh-Hans";
    let is_traditional = settings.selected_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        settings.selected_language
    );

    // Use OpenCC to convert based on selected language
    let config = if is_simplified {
        // Convert Traditional Chinese to Simplified Chinese
        BuiltinConfig::Tw2sp
    } else {
        // Convert Simplified Chinese to Traditional Chinese
        BuiltinConfig::S2twp
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

fn should_show_structuring_overlay(settings: &AppSettings) -> bool {
    if settings.selected_language == "zh-Hans" || settings.selected_language == "zh-Hant" {
        return true;
    }

    if !settings.post_process_enabled {
        return false;
    }

    let Some(provider) = settings.active_post_process_provider() else {
        return false;
    };

    if provider.id != "anamedi" {
        let model = settings
            .post_process_models
            .get(&provider.id)
            .map(String::as_str)
            .unwrap_or("");
        if model.trim().is_empty() {
            return false;
        }
    }

    let Some(prompt_id) = &settings.post_process_selected_prompt_id else {
        return false;
    };

    settings
        .post_process_prompts
        .iter()
        .find(|prompt| &prompt.id == prompt_id)
        .map(|prompt| !prompt.prompt.trim().is_empty())
        .unwrap_or(false)
}

pub async fn process_transcription_result(
    settings: &AppSettings,
    transcription: &str,
    audio_samples: &[f32],
) -> ProcessedTranscription {
    let mut final_text = transcription.to_string();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    if transcription.is_empty() {
        return ProcessedTranscription {
            final_text,
            post_processed_text,
            post_process_prompt,
        };
    }

    if let Some(converted_text) = maybe_convert_chinese_variant(settings, transcription).await {
        final_text = converted_text;
    }

    if let Some(processed_text) =
        maybe_post_process_transcription(settings, &final_text, audio_samples).await
    {
        if processed_text.trim().is_empty() {
            debug!("Post-processing returned empty output; keeping transcription result");
        } else {
            post_processed_text = Some(processed_text.clone());
            final_text = processed_text;

            if let Some(prompt_id) = &settings.post_process_selected_prompt_id {
                if let Some(prompt) = settings
                    .post_process_prompts
                    .iter()
                    .find(|p| &p.id == prompt_id)
                {
                    post_process_prompt = Some(prompt.prompt.clone());
                }
            }
        }
    } else if final_text != transcription {
        post_processed_text = Some(final_text.clone());
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

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        tm.initiate_model_load();

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);
        show_recording_overlay(app);

        let rm = app.state::<Arc<AudioRecordingManager>>();

        // Get the microphone mode to determine audio feedback timing
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_started = false;
        if is_always_on {
            // Always-on mode: Play audio feedback immediately, then apply mute after sound finishes
            debug!("Always-on mode: Playing audio feedback immediately");
            let rm_clone = Arc::clone(&rm);
            let app_clone = app.clone();
            // The blocking helper exits immediately if audio feedback is disabled,
            // so we can always reuse this thread to ensure mute happens right after playback.
            std::thread::spawn(move || {
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });

            recording_started = rm.try_start_recording(&binding_id);
            debug!("Recording started: {}", recording_started);
        } else {
            // On-demand mode: Start recording first, then play audio feedback, then apply mute
            // This allows the microphone to be activated before playing the sound
            debug!("On-demand mode: Starting recording first, then audio feedback");
            let recording_start_time = Instant::now();
            if rm.try_start_recording(&binding_id) {
                recording_started = true;
                debug!("Recording started in {:?}", recording_start_time.elapsed());
                // Small delay to ensure microphone stream is active
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    debug!("Handling delayed audio feedback/mute sequence");
                    // Helper handles disabled audio feedback by returning early, so we reuse it
                    // to keep mute sequencing consistent in every mode.
                    play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    rm_clone.apply_mute();
                });
            } else {
                debug!("Failed to start recording");
            }
        }

        if recording_started {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_transcribing_overlay(app);

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task

        tauri::async_runtime::spawn(async move {
            let binding_id = binding_id.clone(); // Clone for the inner async task
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            if let Some(recording) = rm.stop_recording(&binding_id) {
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    recording.samples.len()
                );

                let history_entry_id = match hm.save_recording(&recording.samples).await {
                    Ok(entry_id) => Some(entry_id),
                    Err(err) => {
                        error!("Failed to save recording before transcription: {}", err);
                        None
                    }
                };
                let transcription_time = Instant::now();
                let post_process_samples = recording.samples.clone();
                match tm.transcribe(&recording) {
                    Ok(transcription) => {
                        debug!(
                            "Transcription completed in {:?}: '{}'",
                            transcription_time.elapsed(),
                            transcription
                        );
                        if !transcription.is_empty() {
                            let settings = get_settings(&ah);
                            if should_show_structuring_overlay(&settings) {
                                show_structuring_overlay(&ah);
                            }
                            let processed = process_transcription_result(
                                &settings,
                                &transcription,
                                &post_process_samples,
                            )
                            .await;

                            if let Some(entry_id) = history_entry_id {
                                if let Err(err) = hm
                                    .complete_transcription(
                                        entry_id,
                                        transcription.clone(),
                                        processed.post_processed_text.clone(),
                                        processed.post_process_prompt.clone(),
                                    )
                                    .await
                                {
                                    error!("Failed to complete transcription in history: {}", err);
                                }
                            }

                            // Paste the final text (either processed or original)
                            let ah_clone = ah.clone();
                            let paste_time = Instant::now();
                            ah.run_on_main_thread(move || {
                                match utils::paste(processed.final_text, ah_clone.clone()) {
                                    Ok(()) => debug!(
                                        "Text pasted successfully in {:?}",
                                        paste_time.elapsed()
                                    ),
                                    Err(e) => error!("Failed to paste transcription: {}", e),
                                }
                                // Hide the overlay after transcription is complete
                                utils::hide_recording_overlay(&ah_clone);
                                change_tray_icon(&ah_clone, TrayIconState::Idle);
                            })
                            .unwrap_or_else(|e| {
                                error!("Failed to run paste on main thread: {:?}", e);
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                            });
                        } else {
                            debug!("Transcription completed with empty output");
                            if let Some(entry_id) = history_entry_id {
                                if let Err(err) = hm
                                    .fail_transcription(
                                        entry_id,
                                        "No speech detected in recording.".to_string(),
                                    )
                                    .await
                                {
                                    error!("Failed to mark empty transcription as failed: {}", err);
                                }
                            }
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                    Err(err) => {
                        debug!("Global Shortcut Transcription error: {}", err);
                        if let Some(entry_id) = history_entry_id {
                            if let Err(history_err) =
                                hm.fail_transcription(entry_id, err.to_string()).await
                            {
                                error!(
                                    "Failed to mark transcription failure in history: {}",
                                    history_err
                                );
                            }
                        }
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }

            // Clear toggle state now that transcription is complete
            if let Ok(mut states) = ah.state::<ManagedToggleState>().lock() {
                states.active_toggles.insert(binding_id, false);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
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
        Arc::new(TranscribeAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});

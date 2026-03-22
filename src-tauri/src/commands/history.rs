use crate::actions::process_transcription_result;
use crate::audio_toolkit::RecordedAudio;
use crate::managers::history::{HistoryEntry, HistoryManager};
use crate::managers::transcription::TranscriptionManager;
use std::sync::Arc;
use tauri::{AppHandle, State};

fn read_saved_recording_samples(file_path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(file_path)
        .map_err(|e| format!("Failed to open saved recording: {}", e))?;
    let spec = reader.spec();
    let channel_count = usize::from(spec.channels.max(1));

    let mono_samples = match spec.sample_format {
        hound::SampleFormat::Int => {
            let raw_samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|sample| {
                    sample
                        .map(|value| value as f32 / i16::MAX as f32)
                        .map_err(|e| format!("Failed to read saved recording sample: {}", e))
                })
                .collect::<Result<Vec<f32>, String>>()?;

            if channel_count == 1 {
                raw_samples
            } else {
                raw_samples
                    .chunks(channel_count)
                    .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
                    .collect()
            }
        }
        hound::SampleFormat::Float => {
            let raw_samples: Vec<f32> = reader
                .samples::<f32>()
                .map(|sample| {
                    sample.map_err(|e| format!("Failed to read saved recording sample: {}", e))
                })
                .collect::<Result<Vec<f32>, String>>()?;

            if channel_count == 1 {
                raw_samples
            } else {
                raw_samples
                    .chunks(channel_count)
                    .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
                    .collect()
            }
        }
    };

    Ok(mono_samples)
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<Vec<HistoryEntry>, String> {
    history_manager
        .get_history_entries()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    if entry.status == crate::managers::history::HistoryEntryStatus::Pending {
        return Err("Transcription is already in progress for this recording.".to_string());
    }

    let file_path = history_manager.get_audio_file_path(&entry.file_name);
    if !file_path.exists() {
        history_manager
            .fail_transcription(id, "Saved recording file could not be found.".to_string())
            .await
            .map_err(|e| e.to_string())?;
        return Err("Saved recording file could not be found.".to_string());
    }

    history_manager
        .mark_transcription_pending(id)
        .await
        .map_err(|e| e.to_string())?;

    let file_path_for_read = file_path.clone();
    let audio_samples = tauri::async_runtime::spawn_blocking(move || {
        read_saved_recording_samples(&file_path_for_read)
    })
    .await
    .map_err(|e| format!("Failed to join audio loading task: {}", e))??;

    transcription_manager.initiate_model_load();
    let transcription_manager = Arc::clone(&transcription_manager);
    let recording = RecordedAudio {
        samples: audio_samples.clone(),
        speech_segments: Vec::new(),
    };
    let transcription =
        tauri::async_runtime::spawn_blocking(move || transcription_manager.transcribe(&recording))
            .await
            .map_err(|e| format!("Failed to join transcription task: {}", e))?;

    let transcription = match transcription {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            history_manager
                .fail_transcription(id, "No speech detected in recording.".to_string())
                .await
                .map_err(|e| e.to_string())?;
            return Err("No speech detected in recording.".to_string());
        }
        Err(err) => {
            history_manager
                .fail_transcription(id, err.to_string())
                .await
                .map_err(|e| e.to_string())?;
            return Err(err.to_string());
        }
    };

    let settings = crate::settings::get_settings(&app);
    let processed = process_transcription_result(&settings, &transcription, &audio_samples).await;
    history_manager
        .complete_transcription(
            id,
            transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days3" => RecordingRetentionPeriod::Days3,
        "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

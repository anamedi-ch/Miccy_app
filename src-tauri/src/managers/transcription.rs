use crate::audio_toolkit::{apply_custom_words, filter_transcription_output, RecordedAudio};
use crate::managers::model::{EngineType, ModelManager};
use crate::settings::{get_settings, AppSettings, ModelUnloadTimeout};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use transcribe_rs::{
    engines::{
        moonshine::{ModelVariant, MoonshineEngine, MoonshineModelParams},
        parakeet::{
            ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity,
        },
        whisper::{WhisperEngine, WhisperInferenceParams},
    },
    TranscriptionEngine,
};

const TRANSCRIPTION_SAMPLE_RATE: usize = 16_000;
const WHISPERX_MAX_CHUNK_SECONDS: usize = 30;
const WHISPERX_INTER_SEGMENT_PADDING_MS: usize = 0;
const DEFAULT_FORCED_SPLIT_OVERLAP_MS: usize = 400;
const DEFAULT_OVERLAP_DEDUPE_WORDS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChunkingConfig {
    max_chunk_samples: usize,
    inter_segment_padding_samples: usize,
    forced_split_overlap_samples: usize,
    overlap_dedupe_words: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct TranscriptionChunk {
    samples: Vec<f32>,
    dedupe_with_previous: bool,
}

fn whisper_chunking_config() -> ChunkingConfig {
    ChunkingConfig {
        max_chunk_samples: TRANSCRIPTION_SAMPLE_RATE * WHISPERX_MAX_CHUNK_SECONDS,
        inter_segment_padding_samples: TRANSCRIPTION_SAMPLE_RATE
            * WHISPERX_INTER_SEGMENT_PADDING_MS
            / 1000,
        forced_split_overlap_samples: TRANSCRIPTION_SAMPLE_RATE * DEFAULT_FORCED_SPLIT_OVERLAP_MS
            / 1000,
        overlap_dedupe_words: DEFAULT_OVERLAP_DEDUPE_WORDS,
    }
}

fn parakeet_chunking_config() -> ChunkingConfig {
    whisper_chunking_config()
}

fn moonshine_chunking_config() -> ChunkingConfig {
    whisper_chunking_config()
}

fn chunking_config_for_engine(engine: &LoadedEngine) -> ChunkingConfig {
    match engine {
        LoadedEngine::Whisper(_) => whisper_chunking_config(),
        LoadedEngine::Parakeet(_) => parakeet_chunking_config(),
        LoadedEngine::Moonshine(_) => moonshine_chunking_config(),
    }
}

fn push_chunk(
    chunks: &mut Vec<TranscriptionChunk>,
    chunk: &mut Vec<f32>,
    dedupe_with_previous: bool,
) {
    if chunk.is_empty() {
        return;
    }
    chunks.push(TranscriptionChunk {
        samples: std::mem::take(chunk),
        dedupe_with_previous,
    });
}

fn append_segment(chunk: &mut Vec<f32>, segment: &[f32], inter_segment_padding_samples: usize) {
    if chunk.is_empty() {
        chunk.extend_from_slice(segment);
        return;
    }
    chunk.extend(std::iter::repeat_n(0.0, inter_segment_padding_samples));
    chunk.extend_from_slice(segment);
}

fn split_long_segment(segment: &[f32], config: ChunkingConfig) -> Vec<TranscriptionChunk> {
    let mut chunks: Vec<TranscriptionChunk> = Vec::new();
    let overlap_samples = config
        .forced_split_overlap_samples
        .min(config.max_chunk_samples.saturating_sub(1));
    let step_samples = config
        .max_chunk_samples
        .saturating_sub(overlap_samples)
        .max(1);
    let mut start = 0;
    let mut dedupe_with_previous = false;

    while start < segment.len() {
        let end = (start + config.max_chunk_samples).min(segment.len());
        chunks.push(TranscriptionChunk {
            samples: segment[start..end].to_vec(),
            dedupe_with_previous,
        });
        if end == segment.len() {
            break;
        }
        start += step_samples;
        dedupe_with_previous = true;
    }

    chunks
}

fn build_transcription_chunks(
    recording: &RecordedAudio,
    config: ChunkingConfig,
) -> Vec<TranscriptionChunk> {
    if recording.speech_segments.is_empty() {
        return split_long_segment(&recording.samples, config);
    }

    let mut chunks: Vec<TranscriptionChunk> = Vec::new();
    let mut current_chunk: Vec<f32> = Vec::new();

    for segment in &recording.speech_segments {
        if segment.len() > config.max_chunk_samples {
            push_chunk(&mut chunks, &mut current_chunk, false);
            chunks.extend(split_long_segment(segment, config));
            continue;
        }

        let padding_len = if current_chunk.is_empty() {
            0
        } else {
            config.inter_segment_padding_samples
        };
        let next_chunk_len = current_chunk.len() + padding_len + segment.len();

        if next_chunk_len > config.max_chunk_samples {
            push_chunk(&mut chunks, &mut current_chunk, false);
        }

        append_segment(
            &mut current_chunk,
            segment,
            config.inter_segment_padding_samples,
        );
    }

    push_chunk(&mut chunks, &mut current_chunk, false);
    chunks
}

fn normalize_word(word: &str) -> String {
    word.trim_matches(|character: char| !character.is_alphanumeric())
        .to_lowercase()
}

fn dedupe_chunk_boundary(
    previous_text: &str,
    current_text: &str,
    max_overlap_words: usize,
) -> String {
    let current_words: Vec<&str> = current_text.split_whitespace().collect();
    if current_words.is_empty() {
        return String::new();
    }

    let previous_words: Vec<&str> = previous_text.split_whitespace().collect();
    let max_overlap = max_overlap_words
        .min(previous_words.len())
        .min(current_words.len());

    for overlap_size in (1..=max_overlap).rev() {
        let previous_window = &previous_words[previous_words.len() - overlap_size..];
        let current_window = &current_words[..overlap_size];

        let is_match = previous_window.iter().zip(current_window.iter()).all(
            |(previous_word, current_word)| {
                normalize_word(previous_word) == normalize_word(current_word)
            },
        );

        if is_match {
            return current_words[overlap_size..].join(" ");
        }
    }

    current_words.join(" ")
}

fn combine_chunk_texts(chunk_texts: Vec<(String, bool)>, config: ChunkingConfig) -> String {
    let mut combined_text = String::new();

    for (chunk_text, dedupe_with_previous) in chunk_texts {
        let trimmed_text = chunk_text.trim();
        if trimmed_text.is_empty() {
            continue;
        }

        let next_text = if dedupe_with_previous {
            dedupe_chunk_boundary(&combined_text, trimmed_text, config.overlap_dedupe_words)
        } else {
            trimmed_text.to_string()
        };

        if next_text.trim().is_empty() {
            continue;
        }

        if !combined_text.is_empty() {
            combined_text.push(' ');
        }
        combined_text.push_str(next_text.trim());
    }

    combined_text
}

fn transcribe_chunk(
    engine: &mut LoadedEngine,
    chunk: &[f32],
    settings: &AppSettings,
) -> Result<String> {
    let text = match engine {
        LoadedEngine::Whisper(whisper_engine) => {
            let whisper_language = if settings.selected_language == "auto" {
                None
            } else {
                let normalized = if settings.selected_language == "zh-Hans"
                    || settings.selected_language == "zh-Hant"
                {
                    "zh".to_string()
                } else {
                    settings.selected_language.clone()
                };
                Some(normalized)
            };
            let params = WhisperInferenceParams {
                language: whisper_language,
                translate: settings.translate_to_english,
                ..Default::default()
            };
            whisper_engine
                .transcribe_samples(chunk.to_vec(), Some(params))
                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                .text
        }
        LoadedEngine::Parakeet(parakeet_engine) => {
            let params = ParakeetInferenceParams {
                timestamp_granularity: TimestampGranularity::Segment,
                ..Default::default()
            };
            parakeet_engine
                .transcribe_samples(chunk.to_vec(), Some(params))
                .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                .text
        }
        LoadedEngine::Moonshine(moonshine_engine) => {
            moonshine_engine
                .transcribe_samples(chunk.to_vec(), None)
                .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e))?
                .text
        }
    };

    Ok(text)
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetEngine),
    Moonshine(MoonshineEngine),
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        };

        // Start the idle watcher
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10)); // Check every 10 seconds

                    // Check shutdown signal again after sleep
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout_seconds = settings.model_unload_timeout.to_seconds();

                    if let Some(limit_seconds) = timeout_seconds {
                        // Skip polling-based unloading for immediate timeout since it's handled directly in transcribe()
                        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
                            continue;
                        }

                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        if now_ms.saturating_sub(last) > limit_seconds * 1000 {
                            // idle -> unload
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                debug!("Starting to unload model due to inactivity");

                                if let Ok(()) = manager_cloned.unload_model() {
                                    let _ = app_handle_cloned.emit(
                                        "model-state-changed",
                                        ModelStateEvent {
                                            event_type: "unloaded".to_string(),
                                            model_id: None,
                                            model_name: None,
                                            error: None,
                                        },
                                    );
                                    let unload_duration = unload_start.elapsed();
                                    debug!(
                                        "Model unloaded due to inactivity (took {}ms)",
                                        unload_duration.as_millis()
                                    );
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    pub fn is_model_loaded(&self) -> bool {
        let engine = self.engine.lock().unwrap();
        engine.is_some()
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.engine.lock().unwrap();
            if let Some(ref mut loaded_engine) = *engine {
                match loaded_engine {
                    LoadedEngine::Whisper(ref mut e) => e.unload_model(),
                    LoadedEngine::Parakeet(ref mut e) => e.unload_model(),
                    LoadedEngine::Moonshine(ref mut e) => e.unload_model(),
                }
            }
            *engine = None; // Drop the engine to free memory
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        // Emit unloaded event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    /// Unloads the model immediately if the setting is enabled and the model is loaded
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        // Emit loading started event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        // Create appropriate engine based on model type
        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let mut engine = WhisperEngine::new();
                engine.load_model(&model_path).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "loading_failed".to_string(),
                            model_id: Some(model_id.to_string()),
                            model_name: Some(model_info.name.clone()),
                            error: Some(error_msg.clone()),
                        },
                    );
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let mut engine = ParakeetEngine::new();
                engine
                    .load_model_with_params(&model_path, ParakeetModelParams::int8())
                    .map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        let _ = self.app_handle.emit(
                            "model-state-changed",
                            ModelStateEvent {
                                event_type: "loading_failed".to_string(),
                                model_id: Some(model_id.to_string()),
                                model_name: Some(model_info.name.clone()),
                                error: Some(error_msg.clone()),
                            },
                        );
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
            EngineType::Moonshine => {
                let mut engine = MoonshineEngine::new();
                engine
                    .load_model_with_params(
                        &model_path,
                        MoonshineModelParams::variant(ModelVariant::Base),
                    )
                    .map_err(|e| {
                        let error_msg =
                            format!("Failed to load moonshine model {}: {}", model_id, e);
                        let _ = self.app_handle.emit(
                            "model-state-changed",
                            ModelStateEvent {
                                event_type: "loading_failed".to_string(),
                                model_id: Some(model_id.to_string()),
                                model_name: Some(model_info.name.clone()),
                                error: Some(error_msg.clone()),
                            },
                        );
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Moonshine(engine)
            }
        };

        // Update the current engine and model ID
        {
            let mut engine = self.engine.lock().unwrap();
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        // Emit loading completed event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    /// Kicks off the model loading in a background thread if it's not already loaded
    pub fn initiate_model_load(&self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading || self.is_model_loaded() {
            return;
        }

        *is_loading = true;
        let self_clone = self.clone();
        thread::spawn(move || {
            let settings = get_settings(&self_clone.app_handle);
            if let Err(e) = self_clone.load_model(&settings.selected_model) {
                error!("Failed to load model: {}", e);
            }
            let mut is_loading = self_clone.is_loading.lock().unwrap();
            *is_loading = false;
            self_clone.loading_condvar.notify_all();
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self.current_model_id.lock().unwrap();
        current_model.clone()
    }

    pub fn transcribe(&self, recording: &RecordedAudio) -> Result<String> {
        // Update last activity timestamp
        self.last_activity.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", recording.samples.len());

        if recording.samples.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        // Check if model is loaded, if not try to load it
        {
            // If the model is loading, wait for it to complete.
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.engine.lock().unwrap();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        // Get current settings for configuration
        let settings = get_settings(&self.app_handle);

        // Perform transcription with the appropriate engine
        let combined_result = {
            let mut engine_guard = self.engine.lock().unwrap();
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!(
                    "Model failed to load after auto-load attempt. Please check your model settings."
                )
            })?;
            let chunking_config = chunking_config_for_engine(engine);
            let chunks = build_transcription_chunks(recording, chunking_config);
            let chunk_count = chunks.len();
            let mut chunk_texts: Vec<(String, bool)> = Vec::new();

            for (chunk_index, chunk) in chunks.iter().enumerate() {
                debug!(
                    "Transcribing chunk {}/{} ({} samples)",
                    chunk_index + 1,
                    chunk_count,
                    chunk.samples.len()
                );
                let chunk_text = transcribe_chunk(engine, &chunk.samples, &settings)?;
                chunk_texts.push((chunk_text, chunk.dedupe_with_previous));
            }

            combine_chunk_texts(chunk_texts, chunking_config)
        };

        // Apply word correction if custom words are configured
        let corrected_result = if !settings.custom_words.is_empty() {
            apply_custom_words(
                &combined_result,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            combined_result
        };

        // Filter out filler words and hallucinations
        let filtered_result = filter_transcription_output(&corrected_result);

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        let final_result = filtered_result;

        if final_result.is_empty() {
            info!("Transcription result is empty");
        } else {
            info!("Transcription result: {}", final_result);
        }

        self.maybe_unload_immediately("transcription");

        Ok(final_result)
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        debug!("Shutting down TranscriptionManager");

        // Signal the watcher thread to shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the thread to finish gracefully
        if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_transcription_chunks, combine_chunk_texts, dedupe_chunk_boundary,
        whisper_chunking_config, RecordedAudio, TRANSCRIPTION_SAMPLE_RATE,
    };

    fn build_samples(seconds: usize) -> Vec<f32> {
        vec![1.0; seconds * TRANSCRIPTION_SAMPLE_RATE]
    }

    #[test]
    fn merges_short_vad_segments_without_padding_like_whisperx() {
        let config = whisper_chunking_config();
        let first_segment = build_samples(10);
        let second_segment = build_samples(5);
        let recording = RecordedAudio {
            samples: [first_segment.clone(), second_segment.clone()].concat(),
            speech_segments: vec![first_segment, second_segment],
        };

        let chunks = build_transcription_chunks(&recording, config);

        assert_eq!(chunks.len(), 1);
        assert_eq!(config.inter_segment_padding_samples, 0);
        assert_eq!(chunks[0].samples.len(), 15 * TRANSCRIPTION_SAMPLE_RATE);
        assert!(!chunks[0].dedupe_with_previous);
    }

    #[test]
    fn starts_new_chunk_at_silence_boundary_when_limit_is_exceeded() {
        let config = whisper_chunking_config();
        let first_segment = build_samples(20);
        let second_segment = build_samples(12);
        let recording = RecordedAudio {
            samples: [first_segment.clone(), second_segment.clone()].concat(),
            speech_segments: vec![first_segment.clone(), second_segment.clone()],
        };

        let chunks = build_transcription_chunks(&recording, config);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].samples.len(), first_segment.len());
        assert_eq!(chunks[1].samples.len(), second_segment.len());
        assert!(!chunks[0].dedupe_with_previous);
        assert!(!chunks[1].dedupe_with_previous);
    }

    #[test]
    fn splits_uninterrupted_speech_with_overlap_and_marks_follow_up_chunks_for_dedupe() {
        let config = whisper_chunking_config();
        let long_segment = build_samples(35);
        let recording = RecordedAudio {
            samples: long_segment.clone(),
            speech_segments: vec![long_segment],
        };

        let chunks = build_transcription_chunks(&recording, config);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].samples.len(), config.max_chunk_samples);
        assert_eq!(
            chunks[1].samples.len(),
            (5 * TRANSCRIPTION_SAMPLE_RATE) + config.forced_split_overlap_samples
        );
        assert!(!chunks[0].dedupe_with_previous);
        assert!(chunks[1].dedupe_with_previous);
    }

    #[test]
    fn falls_back_to_raw_samples_when_vad_segments_are_unavailable() {
        let config = whisper_chunking_config();
        let recording = RecordedAudio {
            samples: build_samples(65),
            speech_segments: Vec::new(),
        };

        let chunks = build_transcription_chunks(&recording, config);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].samples.len(), config.max_chunk_samples);
        assert_eq!(chunks[1].samples.len(), config.max_chunk_samples);
        assert_eq!(
            chunks[2].samples.len(),
            (5 * TRANSCRIPTION_SAMPLE_RATE) + (2 * config.forced_split_overlap_samples)
        );
    }

    #[test]
    fn removes_overlapping_words_when_joining_forced_split_chunks() {
        let deduped_text = dedupe_chunk_boundary(
            "hello world this is a test",
            "is a test of the boundary dedupe",
            8,
        );

        assert_eq!(deduped_text, "of the boundary dedupe");
    }

    #[test]
    fn keeps_non_overlapping_chunk_text_unchanged() {
        let combined = combine_chunk_texts(
            vec![
                ("hello world".to_string(), false),
                ("fresh start".to_string(), true),
            ],
            whisper_chunking_config(),
        );

        assert_eq!(combined, "hello world fresh start");
    }
}

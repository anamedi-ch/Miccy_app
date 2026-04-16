use log::{debug, warn};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

impl From<LogLevel> for tauri_plugin_log::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
            LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
            LogLevel::Info => tauri_plugin_log::LogLevel::Info,
            LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
            LogLevel::Error => tauri_plugin_log::LogLevel::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMPrompt {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_base_url_edit: bool,
    #[serde(default)]
    pub models_endpoint: Option<String>,
}

/// CPU/GPU trade-off for the bundled local summarization runtime (`llama-server`).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, Type)]
#[serde(rename_all = "snake_case")]
pub enum LocalLlmPerformancePreset {
    Low,
    #[default]
    Default,
    High,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec5, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    DontModify,
    CopyToClipboard,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardImplementation {
    Tauri,
    HandyKeys,
}

impl Default for KeyboardImplementation {
    fn default() -> Self {
        // Default to HandyKeys only on macOS where it's well-tested.
        // Windows and Linux use Tauri by default (handy-keys not sufficiently tested yet).
        #[cfg(target_os = "macos")]
        return KeyboardImplementation::HandyKeys;
        #[cfg(not(target_os = "macos"))]
        return KeyboardImplementation::Tauri;
    }
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Never
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Default to CtrlV for macOS and Windows, Direct for Linux
        #[cfg(target_os = "linux")]
        return PasteMethod::Direct;
        #[cfg(not(target_os = "linux"))]
        return PasteMethod::CtrlV;
    }
}

impl Default for ClipboardHandling {
    fn default() -> Self {
        ClipboardHandling::DontModify
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec5 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec5 => Some(5),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(&self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(&self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(&self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

/* still handy for composing the initial JSON in the store ------------- */
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_post_process_enabled")]
    pub post_process_enabled: bool,
    #[serde(default = "default_post_process_provider_id")]
    pub post_process_provider_id: String,
    #[serde(default = "default_post_process_providers")]
    pub post_process_providers: Vec<PostProcessProvider>,
    #[serde(default = "default_post_process_api_keys")]
    pub post_process_api_keys: HashMap<String, String>,
    #[serde(default = "default_post_process_models")]
    pub post_process_models: HashMap<String, String>,
    #[serde(default = "default_post_process_prompts")]
    pub post_process_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub post_process_selected_prompt_id: Option<String>,
    /// Context window for Ollama post-processing (`num_ctx`). Clamped to
    /// [`OLLAMA_POST_PROCESS_NUM_CTX_MIN`], [`OLLAMA_POST_PROCESS_NUM_CTX_MAX`].
    #[serde(default = "default_post_process_ollama_num_ctx")]
    pub post_process_ollama_num_ctx: u32,
    /// Max new tokens from Ollama (`num_predict`). `0` = omit (model default, no cap).
    #[serde(default = "default_post_process_ollama_num_predict")]
    pub post_process_ollama_num_predict: u32,
    #[serde(default)]
    pub post_process_local_performance: LocalLlmPerformancePreset,
    /// Context window (`-c`) for bundled `llama-server`. Clamped to
    /// [`LOCAL_POST_PROCESS_CTX_MIN`], [`LOCAL_POST_PROCESS_CTX_MAX`].
    #[serde(default = "default_post_process_local_ctx")]
    pub post_process_local_ctx: u32,
    /// Max completion tokens for local server (`max_tokens`). `0` = omit (uncapped).
    #[serde(default = "default_post_process_local_max_tokens")]
    pub post_process_local_max_tokens: u32,
    /// Sampling temperature for local post-processing (OpenAI `temperature`).
    #[serde(default = "default_post_process_local_temperature")]
    pub post_process_local_temperature: f64,
    /// Minutes after a completed local post-process before stopping `llama-server`. `0` = keep running.
    #[serde(default = "default_post_process_local_idle_shutdown_minutes")]
    pub post_process_local_idle_shutdown_minutes: u32,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default)]
    pub experimental_enabled: bool,
    #[serde(default)]
    pub keyboard_implementation: KeyboardImplementation,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
}

fn default_model() -> String {
    "".to_string()
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

fn default_start_hidden() -> bool {
    false
}

fn default_autostart_enabled() -> bool {
    false
}

fn default_update_checks_enabled() -> bool {
    true
}

fn default_selected_language() -> String {
    "auto".to_string()
}

fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

fn default_log_level() -> LogLevel {
    LogLevel::Debug
}

fn default_word_correction_threshold() -> f64 {
    0.18
}

fn default_paste_delay_ms() -> u64 {
    60
}

fn default_history_limit() -> usize {
    5
}

fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

fn default_audio_feedback_volume() -> f32 {
    1.0
}

fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

fn default_post_process_enabled() -> bool {
    false
}

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn default_post_process_provider_id() -> String {
    "custom".to_string()
}

fn default_post_process_providers() -> Vec<PostProcessProvider> {
    vec![
        PostProcessProvider {
            id: "local_private".to_string(),
            label: "This computer (private)".to_string(),
            base_url: "http://127.0.0.1:1/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: None,
        },
        PostProcessProvider {
            id: "custom".to_string(),
            label: "Custom Local".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            allow_base_url_edit: true,
            models_endpoint: Some("/models".to_string()),
        },
    ]
}

fn default_post_process_api_keys() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(provider.id, String::new());
    }
    map
}

fn default_post_process_models() -> HashMap<String, String> {
    HashMap::new()
}

/// Minimum `num_ctx` for Ollama structuring (fits prompt + short transcript).
pub const OLLAMA_POST_PROCESS_NUM_CTX_MIN: u32 = 2048;
/// Maximum `num_ctx` sent to Ollama for post-processing.
pub const OLLAMA_POST_PROCESS_NUM_CTX_MAX: u32 = 131_072;
/// Default `num_ctx`: ~15–20 min German dictation + instructions without truncation.
pub const DEFAULT_POST_PROCESS_OLLAMA_NUM_CTX: u32 = 12_288;
/// Upper bound when `num_predict` is set (non-zero).
pub const OLLAMA_POST_PROCESS_NUM_PREDICT_MAX: u32 = 131_072;

/// Minimum context (`-c`) for bundled `llama-server` post-processing.
pub const LOCAL_POST_PROCESS_CTX_MIN: u32 = 2048;
/// Maximum context for local post-processing.
pub const LOCAL_POST_PROCESS_CTX_MAX: u32 = 131_072;
/// Default local context: matches Ollama default intent (~15–20 min dictation + prompt).
pub const DEFAULT_POST_PROCESS_LOCAL_CTX: u32 = 12_288;

pub const LOCAL_POST_PROCESS_TEMPERATURE_MIN: f64 = 0.0;
pub const LOCAL_POST_PROCESS_TEMPERATURE_MAX: f64 = 2.0;
/// Slightly conservative default for clinical structuring.
pub const DEFAULT_POST_PROCESS_LOCAL_TEMPERATURE: f64 = 0.35;

/// Stop `llama-server` after this many minutes with no completed post-process. `0` = never.
pub const LOCAL_POST_PROCESS_IDLE_SHUTDOWN_MAX_MINUTES: u32 = 720;
pub const DEFAULT_POST_PROCESS_LOCAL_IDLE_SHUTDOWN_MINUTES: u32 = 15;

fn default_post_process_ollama_num_ctx() -> u32 {
    DEFAULT_POST_PROCESS_OLLAMA_NUM_CTX
}

fn default_post_process_ollama_num_predict() -> u32 {
    0
}

fn default_post_process_local_ctx() -> u32 {
    DEFAULT_POST_PROCESS_LOCAL_CTX
}

fn default_post_process_local_max_tokens() -> u32 {
    0
}

fn default_post_process_local_temperature() -> f64 {
    DEFAULT_POST_PROCESS_LOCAL_TEMPERATURE
}

fn default_post_process_local_idle_shutdown_minutes() -> u32 {
    DEFAULT_POST_PROCESS_LOCAL_IDLE_SHUTDOWN_MINUTES
}

/// Clamp stored `num_ctx` to supported range.
pub fn clamp_ollama_post_process_num_ctx(raw: u32) -> u32 {
    raw.clamp(
        OLLAMA_POST_PROCESS_NUM_CTX_MIN,
        OLLAMA_POST_PROCESS_NUM_CTX_MAX,
    )
}

/// `None` means do not send `num_predict` (model default / uncapped generation).
pub fn normalize_ollama_post_process_num_predict(raw: u32) -> Option<u32> {
    if raw == 0 {
        None
    } else {
        Some(raw.min(OLLAMA_POST_PROCESS_NUM_PREDICT_MAX))
    }
}

/// Clamp local `llama-server` context size.
pub fn clamp_local_post_process_ctx(raw: u32) -> u32 {
    raw.clamp(LOCAL_POST_PROCESS_CTX_MIN, LOCAL_POST_PROCESS_CTX_MAX)
}

/// Clamp local sampling temperature.
pub fn clamp_local_post_process_temperature(raw: f64) -> f64 {
    raw.clamp(
        LOCAL_POST_PROCESS_TEMPERATURE_MIN,
        LOCAL_POST_PROCESS_TEMPERATURE_MAX,
    )
}

/// `None` means omit `max_tokens` (uncapped). Non-zero values are capped.
pub fn normalize_local_post_process_max_tokens(raw: u32) -> Option<u32> {
    if raw == 0 {
        None
    } else {
        Some(raw.min(OLLAMA_POST_PROCESS_NUM_PREDICT_MAX))
    }
}

pub fn clamp_local_post_process_idle_shutdown_minutes(raw: u32) -> u32 {
    raw.min(LOCAL_POST_PROCESS_IDLE_SHUTDOWN_MAX_MINUTES)
}

fn default_post_process_prompts() -> Vec<LLMPrompt> {
    vec![
        LLMPrompt {
            id: "improve_transcript_en".to_string(),
            name: "Improve Transcript".to_string(),
            description: Some(
                "Cleans up grammar and obvious recognition errors; meaning unchanged.".to_string(),
            ),
            prompt: r#"Rewrite in clear standard English. Keep meaning and facts exactly the same. Fix grammar, spelling, filler words, and obvious speech-recognition mistakes. Do not summarize, add headings, JSON, or commentary.

Transcript:
${output}"#
                .to_string(),
        },
        LLMPrompt {
            id: "summary_bullets_en".to_string(),
            name: "Structured summary (bullet points)".to_string(),
            description: Some("Concise bullet-point summary of the transcript.".to_string()),
            prompt: r#"Output concise English bullet points (each line starts with "- "). Only use content from the transcript: key facts, decisions, findings, next steps, open questions. No introduction, no JSON.

Transcript:
${output}"#
                .to_string(),
        },
        LLMPrompt {
            id: "soap_en".to_string(),
            name: "SOAP (EN)".to_string(),
            description: Some(
                "Clinical note: Subjective, Objective, Assessment, Plan.".to_string(),
            ),
            prompt: r#"Clinical SOAP note from the encounter. Exactly four sections with a plain heading (no colon), then compact lines using "• ". Separate sections with a blank line: Subjective, Objective, Assessment, Plan.

Only explicitly stated information; use clinical terminology where appropriate; otherwise use "• Not stated". No overall title, no JSON.

Transcript:
${output}"#
                .to_string(),
        },
        LLMPrompt {
            id: "transcript_improve".to_string(),
            name: "Transkript verbessern".to_string(),
            description: Some(
                "Verbessert ein Transkript in standardsprachlichem Deutsch ohne inhaltliche Änderungen."
                    .to_string(),
            ),
            prompt: r#"Überarbeiten Sie in klarem Standarddeutsch: Sinn und fachliche Aussage unverändert; Grammatik, Orthographie, Dialekt/Umgangssprache→Standardsprache; offensichtliche Transkriptionsfehler korrigieren. Keine Zusammenfassung, keine Überschriften, kein JSON, kein Kommentar.

Transkript:
${output}"#
                .to_string(),
        },
        LLMPrompt {
            id: "transcript_summarize".to_string(),
            name: "Zusammenfassung (Stichpunkte)".to_string(),
            description: Some(
                "Kompakte deutsche Stichpunkte zum Inhalt des Transkripts.".to_string(),
            ),
            prompt: r#"Kompakte deutsche Stichpunkte (je Zeile ein "- "). Nur Inhalte aus dem Transkript: Wesentliches, Entscheidungen, Befunde, nächste Schritte, offene Punkte. Keine Einleitung, kein JSON.

Transkript:
${output}"#
                .to_string(),
        },
        LLMPrompt {
            id: "soap".to_string(),
            name: "SOAP (DE)".to_string(),
            description: Some(
                "Dokumentation: Subjektiv, Objektiv, Untersuchung, Beurteilung, Procedere."
                    .to_string(),
            ),
            prompt: r#"SOAP aus hausärztlichem Gespräch. Genau fünf Abschnitte, Überschrift ohne Doppelpunkt, darunter nur kompakte Zeilen mit "• ". Abschnitte mit Leerzeile trennen: Subjektiv, Objektiv, Untersuchung, Beurteilung, Procedere.

Nur explizit Genanntes; medizinische Terminologie; sonst "• Keine Angaben". Kein Gesamttitel, kein JSON.

Transkript:
${output}"#
                .to_string(),
        },
    ]
}

fn ensure_post_process_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    let default_providers = default_post_process_providers();

    // Determine the set of allowed provider ids (only local/custom)
    let allowed_ids: std::collections::HashSet<String> = default_providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect();

    // Prune any legacy providers that are no longer shipped (remote APIs, removed cloud tiers, etc.)
    let original_providers_len = settings.post_process_providers.len();
    settings
        .post_process_providers
        .retain(|provider| allowed_ids.contains(&provider.id));
    if settings.post_process_providers.len() != original_providers_len {
        changed = true;
    }

    // Ensure all default providers, API keys, and models are present
    for provider in &default_providers {
        if settings
            .post_process_providers
            .iter()
            .all(|existing| existing.id != provider.id)
        {
            settings.post_process_providers.push(provider.clone());
            changed = true;
        }

        if !settings.post_process_api_keys.contains_key(&provider.id) {
            settings
                .post_process_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        match settings.post_process_models.get_mut(&provider.id) {
            Some(existing) => {
                // Keep existing model configuration as-is for local/custom providers
                let _ = existing;
            }
            None => {
                let default_model = if provider.id == "local_private" {
                    "local_fast".to_string()
                } else {
                    String::new()
                };
                settings
                    .post_process_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }
    }

    // Prune API keys and models for non-allowed providers
    let original_keys_len = settings.post_process_api_keys.len();
    settings
        .post_process_api_keys
        .retain(|id, _| allowed_ids.contains(id));
    if settings.post_process_api_keys.len() != original_keys_len {
        changed = true;
    }

    let original_models_len = settings.post_process_models.len();
    settings
        .post_process_models
        .retain(|id, _| allowed_ids.contains(id));
    if settings.post_process_models.len() != original_models_len {
        changed = true;
    }

    // Ensure the active provider id points to a valid (local/custom) provider
    if !settings
        .post_process_providers
        .iter()
        .any(|provider| provider.id == settings.post_process_provider_id)
    {
        if let Some(default_provider) = default_providers.first() {
            settings.post_process_provider_id = default_provider.id.clone();
            changed = true;
        }
    }

    let default_prompts = default_post_process_prompts();

    // Drop specialty prompts we no longer ship (user custom prompts use ids like "prompt_…").
    const LEGACY_SHIPPED_PROMPT_IDS: &[&str] = &[
        "psychology",
        "soap_special",
        "soap_problems",
        "soap_nephrology",
    ];
    let before_prune = settings.post_process_prompts.len();
    settings
        .post_process_prompts
        .retain(|p| !LEGACY_SHIPPED_PROMPT_IDS.contains(&p.id.as_str()));
    if settings.post_process_prompts.len() != before_prune {
        changed = true;
    }

    // One-time migration: update legacy soap_json_de to new soap prompt BEFORE adding defaults
    // (avoids ending up with both soap and soap_json_de)
    if let Some(existing) = settings.post_process_prompts.iter_mut().find(|prompt| {
        prompt.id == "improve_transcription"
            || prompt.id == "improve_transcript"
            || prompt.name == "Improve Transcription"
    }) {
        if let Some(new_prompt) = default_prompts
            .iter()
            .find(|prompt| prompt.id == "transcript_improve")
        {
            existing.id = new_prompt.id.clone();
            existing.name = new_prompt.name.clone();
            existing.description = new_prompt.description.clone();
            existing.prompt = new_prompt.prompt.clone();
            if matches!(
                settings.post_process_selected_prompt_id.as_deref(),
                Some("improve_transcription" | "improve_transcript")
            ) {
                settings.post_process_selected_prompt_id = Some("transcript_improve".to_string());
            }
            changed = true;
        }
    }

    if let Some(existing) = settings.post_process_prompts.iter_mut().find(|prompt| {
        prompt.id == "summarize_transcript"
            || prompt.id == "summarize_transcript_bullets"
            || prompt.id == "transcript_summary"
            || prompt.name == "Summarize Transcript (Bullets)"
    }) {
        if let Some(new_prompt) = default_prompts
            .iter()
            .find(|prompt| prompt.id == "transcript_summarize")
        {
            existing.id = new_prompt.id.clone();
            existing.name = new_prompt.name.clone();
            existing.description = new_prompt.description.clone();
            existing.prompt = new_prompt.prompt.clone();
            if matches!(
                settings.post_process_selected_prompt_id.as_deref(),
                Some(
                    "summarize_transcript" | "summarize_transcript_bullets" | "transcript_summary"
                )
            ) {
                settings.post_process_selected_prompt_id = Some("transcript_summarize".to_string());
            }
            changed = true;
        }
    }

    if let Some(existing) = settings
        .post_process_prompts
        .iter_mut()
        .find(|prompt| prompt.id == "soap_json_de")
    {
        if let Some(new_soap_prompt) = default_prompts.iter().find(|prompt| prompt.id == "soap") {
            existing.id = new_soap_prompt.id.clone();
            existing.name = new_soap_prompt.name.clone();
            existing.description = new_soap_prompt.description.clone();
            existing.prompt = new_soap_prompt.prompt.clone();
            if settings.post_process_selected_prompt_id.as_deref() == Some("soap_json_de") {
                settings.post_process_selected_prompt_id = Some("soap".to_string());
            }
            changed = true;
        }
    }

    // Remove duplicate built-in prompts - keep only the first occurrence.
    let original_len = settings.post_process_prompts.len();
    let mut seen_builtin_prompt_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    settings.post_process_prompts.retain(|p| {
        if matches!(
            p.id.as_str(),
            "improve_transcript_en"
                | "summary_bullets_en"
                | "soap_en"
                | "transcript_improve"
                | "transcript_summarize"
                | "soap"
        ) {
            if seen_builtin_prompt_ids.contains(&p.id) {
                false
            } else {
                seen_builtin_prompt_ids.insert(p.id.clone());
                true
            }
        } else {
            true
        }
    });
    if settings.post_process_prompts.len() != original_len {
        changed = true;
    }

    // Ensure all default prompts are present
    let existing_prompt_ids: std::collections::HashSet<String> = settings
        .post_process_prompts
        .iter()
        .map(|prompt| prompt.id.clone())
        .collect();

    for default_prompt in &default_prompts {
        if !existing_prompt_ids.contains(&default_prompt.id) {
            settings.post_process_prompts.push(default_prompt.clone());
            changed = true;
        }
    }

    let prompt_order_before = settings
        .post_process_prompts
        .iter()
        .map(|prompt| prompt.id.clone())
        .collect::<Vec<String>>();
    settings
        .post_process_prompts
        .sort_by_key(|prompt| match prompt.id.as_str() {
            "improve_transcript_en" => 0,
            "summary_bullets_en" => 1,
            "soap_en" => 2,
            "transcript_improve" => 3,
            "transcript_summarize" => 4,
            "soap" => 5,
            _ => 100,
        });
    let prompt_order_after = settings
        .post_process_prompts
        .iter()
        .map(|prompt| prompt.id.clone())
        .collect::<Vec<String>>();
    if prompt_order_before != prompt_order_after {
        changed = true;
    }

    if let Some(selected) = settings.post_process_selected_prompt_id.clone() {
        if !settings
            .post_process_prompts
            .iter()
            .any(|p| p.id == selected)
        {
            settings.post_process_selected_prompt_id =
                settings.post_process_prompts.first().map(|p| p.id.clone());
            changed = true;
        }
    }

    changed
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    let default_shortcut = "ctrl+space";
    #[cfg(target_os = "macos")]
    let default_shortcut = "option+space";
    #[cfg(target_os = "linux")]
    let default_shortcut = "ctrl+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_shortcut = "alt+space";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: "auto".to_string(),
        overlay_position: default_overlay_position(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        model_unload_timeout: ModelUnloadTimeout::Never,
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: Some("improve_transcript_en".to_string()),
        post_process_ollama_num_ctx: default_post_process_ollama_num_ctx(),
        post_process_ollama_num_predict: default_post_process_ollama_num_predict(),
        post_process_local_performance: LocalLlmPerformancePreset::default(),
        post_process_local_ctx: default_post_process_local_ctx(),
        post_process_local_max_tokens: default_post_process_local_max_tokens(),
        post_process_local_temperature: default_post_process_local_temperature(),
        post_process_local_idle_shutdown_minutes: default_post_process_local_idle_shutdown_minutes(
        ),
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        experimental_enabled: false,
        keyboard_implementation: KeyboardImplementation::default(),
        paste_delay_ms: default_paste_delay_ms(),
    }
}

impl AppSettings {
    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
    }

    pub fn post_process_provider(&self, provider_id: &str) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn post_process_provider_mut(
        &mut self,
        provider_id: &str,
    ) -> Option<&mut PostProcessProvider> {
        self.post_process_providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if !settings.bindings.contains_key(&key) {
                        debug!("Adding missing binding: {}", key);
                        settings.bindings.insert(key, value);
                        updated = true;
                    }
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                warn!("Failed to parse settings: {}", e);
                // Fall back to default settings if parsing fails
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let default_settings = get_default_settings();
            store.set("settings", serde_json::to_value(&default_settings).unwrap());
            default_settings
        })
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}

#[cfg(test)]
mod local_private_provider_tests {
    use super::ensure_post_process_defaults;
    use super::get_default_settings;
    use super::LocalLlmPerformancePreset;
    use super::PostProcessProvider;

    #[test]
    fn default_post_process_providers_start_with_local_private() {
        let settings = get_default_settings();
        assert_eq!(
            settings
                .post_process_providers
                .first()
                .map(|p| p.id.as_str()),
            Some("local_private")
        );
        assert!(settings
            .post_process_providers
            .iter()
            .any(|p| p.id == "custom" && p.allow_base_url_edit));
    }

    #[test]
    fn default_post_process_providers_exclude_removed_cloud_tier() {
        let settings = get_default_settings();
        assert!(
            !settings
                .post_process_providers
                .iter()
                .any(|p| p.id == "anamedi"),
            "legacy cloud provider id must not ship in defaults"
        );
    }

    #[test]
    fn ensure_post_process_defaults_strips_legacy_anamedi_provider() {
        let mut settings = get_default_settings();
        settings.post_process_providers.push(PostProcessProvider {
            id: "anamedi".to_string(),
            label: "Legacy Cloud".to_string(),
            base_url: "https://app.anamedi.com".to_string(),
            allow_base_url_edit: false,
            models_endpoint: None,
        });
        settings
            .post_process_api_keys
            .insert("anamedi".to_string(), "test-key".to_string());
        settings
            .post_process_models
            .insert("anamedi".to_string(), "m".to_string());
        settings.post_process_provider_id = "anamedi".to_string();

        assert!(ensure_post_process_defaults(&mut settings));

        assert!(!settings
            .post_process_providers
            .iter()
            .any(|p| p.id == "anamedi"));
        assert!(!settings.post_process_api_keys.contains_key("anamedi"));
        assert!(!settings.post_process_models.contains_key("anamedi"));
        assert_ne!(settings.post_process_provider_id, "anamedi");
        let active = settings
            .active_post_process_provider()
            .expect("active provider after migration");
        assert!(active.id == "local_private" || active.id == "custom");
    }

    #[test]
    fn local_llm_performance_preset_defaults_to_default_variant() {
        assert_eq!(
            LocalLlmPerformancePreset::default(),
            LocalLlmPerformancePreset::Default
        );
    }

    #[test]
    fn default_post_process_prompts_are_six_en_then_de() {
        let settings = get_default_settings();
        assert_eq!(settings.post_process_prompts.len(), 6);
        let ids: Vec<&str> = settings
            .post_process_prompts
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "improve_transcript_en",
                "summary_bullets_en",
                "soap_en",
                "transcript_improve",
                "transcript_summarize",
                "soap",
            ]
        );
        assert_eq!(
            settings.post_process_selected_prompt_id.as_deref(),
            Some("improve_transcript_en")
        );
    }

    #[test]
    fn ensure_post_process_defaults_removes_legacy_specialty_prompts() {
        let mut settings = get_default_settings();
        settings.post_process_prompts.push(super::LLMPrompt {
            id: "psychology".to_string(),
            name: "Psychology".to_string(),
            description: None,
            prompt: "legacy".to_string(),
        });
        assert!(ensure_post_process_defaults(&mut settings));
        assert!(!settings
            .post_process_prompts
            .iter()
            .any(|p| p.id == "psychology"));
        assert_eq!(settings.post_process_prompts.len(), 6);
    }
}

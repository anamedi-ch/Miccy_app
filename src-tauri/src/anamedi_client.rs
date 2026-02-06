use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use std::path::Path;

/// Response shape for Anamedi's /api/transcribe-custom-structure endpoint.
#[derive(Debug, Deserialize)]
pub struct AnamediCustomStructureResponse {
    pub transcript: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub diarized: serde_json::Value,
    #[serde(rename = "structuredData")]
    pub structured_data: serde_json::Value,
}

/// Call Anamedi's /api/transcribe-custom-structure with an existing audio file.
///
/// - `api_key`: Anamedi API key (required, x-api-key header)
/// - `audio_path`: Path to a WAV/MP3/etc. audio file
/// - `schema`: JSON schema string defining the desired output structure
/// - `instructions`: Optional additional instructions for structuring
/// - `contact_email`: Optional email for tracking
/// - `language`: Optional ISO 639-3 language code (e.g. "deu", "eng")
pub async fn transcribe_custom_structure_with_file(
    api_key: &str,
    audio_path: &Path,
    schema: &str,
    instructions: Option<&str>,
    contact_email: Option<&str>,
    language: Option<&str>,
) -> Result<AnamediCustomStructureResponse, String> {
    if api_key.trim().is_empty() {
        return Err("Anamedi API key is empty".to_string());
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key)
            .map_err(|e| format!("Invalid Anamedi API key header value: {}", e))?,
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Anamedi/1.0 (+https://github.com/cjpais/Anamedi)"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client for Anamedi: {}", e))?;

    let file_name = audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();

    let file_part = reqwest::multipart::Part::file(audio_path)
        .await
        .map_err(|e| format!("Failed to attach audio file for Anamedi: {}", e))?
        .file_name(file_name);

    let mut form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("schema", schema.to_string());

    if let Some(instructions_text) = instructions {
        if !instructions_text.trim().is_empty() {
            form = form.text("instructions", instructions_text.to_string());
        }
    }

    if let Some(email) = contact_email {
        if !email.trim().is_empty() {
            form = form.text("contactEmail", email.to_string());
        }
    }

    if let Some(lang) = language {
        if !lang.trim().is_empty() {
            form = form.text("language", lang.to_string());
        }
    }

    let url = "https://app.anamedi.com/api/transcribe-custom-structure";
    debug!("Sending Anamedi transcribe-custom-structure request to {}", url);

    let response = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Anamedi request failed: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Anamedi response body: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Anamedi API request failed with status {}: {}",
            status, body
        ));
    }

    let parsed: AnamediCustomStructureResponse =
        serde_json::from_str(&body).map_err(|e| {
            format!(
                "Failed to parse Anamedi response JSON: {}. Body: {}",
                e, body
            )
        })?;

    Ok(parsed)
}


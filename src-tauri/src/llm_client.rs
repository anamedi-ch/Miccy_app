use crate::settings::PostProcessProvider;
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};

const DEFAULT_OLLAMA_NUM_CTX: u32 = 16_384;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ChatCompletionOptions {
    num_ctx: u32,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatCompletionOptions>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

/// Build headers for API requests based on provider type
fn build_headers(provider: &PostProcessProvider, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    // Common headers
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://github.com/anamedi-ch/anamedi_lokal"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Anamedi/1.0 (+https://github.com/anamedi-ch/anamedi_lokal)"),
    );
    headers.insert("X-Title", HeaderValue::from_static("Anamedi Local"));

    // Provider-specific auth headers
    if !api_key.is_empty() {
        if provider.id == "anthropic" {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key)
                    .map_err(|e| format!("Invalid API key header value: {}", e))?,
            );
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| format!("Invalid authorization header value: {}", e))?,
            );
        }
    }

    Ok(headers)
}

/// Create an HTTP client with provider-specific headers
fn create_client(provider: &PostProcessProvider, api_key: &str) -> Result<reqwest::Client, String> {
    let headers = build_headers(provider, api_key)?;
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn uses_ollama_chat_options(provider: &PostProcessProvider) -> bool {
    if provider.id != "custom" {
        return false;
    }

    let Ok(url) = reqwest::Url::parse(provider.base_url.trim()) else {
        return false;
    };

    matches!(url.port_or_known_default(), Some(11434))
}

fn build_chat_completion_request(model: &str, prompt: String, provider: &PostProcessProvider) -> ChatCompletionRequest {
    let options = if uses_ollama_chat_options(provider) {
        Some(ChatCompletionOptions {
            num_ctx: DEFAULT_OLLAMA_NUM_CTX,
        })
    } else {
        None
    };

    ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        options,
    }
}

/// Send a chat completion request to an OpenAI-compatible API
/// Returns Ok(Some(content)) on success, Ok(None) if response has no content,
/// or Err on actual errors (HTTP, parsing, etc.)
pub async fn send_chat_completion(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    prompt: String,
) -> Result<Option<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    debug!("Sending chat completion request to: {}", url);

    let client = create_client(provider, &api_key)?;
    let request_body = build_chat_completion_request(model, prompt, provider);

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        return Err(format!(
            "API request failed with status {}: {}",
            status, error_text
        ));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    Ok(completion
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone()))
}

/// Fetch available models from an OpenAI-compatible API
/// Returns a list of model IDs
pub async fn fetch_models(
    provider: &PostProcessProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    debug!("Fetching models from: {}", url);

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::{build_chat_completion_request, uses_ollama_chat_options, DEFAULT_OLLAMA_NUM_CTX};
    use crate::settings::PostProcessProvider;

    fn build_provider(id: &str, base_url: &str) -> PostProcessProvider {
        PostProcessProvider {
            id: id.to_string(),
            label: "Provider".to_string(),
            base_url: base_url.to_string(),
            allow_base_url_edit: true,
            models_endpoint: Some("/models".to_string()),
        }
    }

    #[test]
    fn detects_default_ollama_openai_endpoint() {
        let provider = build_provider("custom", "http://localhost:11434/v1");

        assert!(uses_ollama_chat_options(&provider));
    }

    #[test]
    fn skips_ollama_options_for_non_ollama_provider() {
        let provider = build_provider("custom", "https://api.openai.com/v1");

        assert!(!uses_ollama_chat_options(&provider));
    }

    #[test]
    fn includes_num_ctx_for_ollama_requests() {
        let provider = build_provider("custom", "http://127.0.0.1:11434/v1");
        let request = build_chat_completion_request("qwen2.5:14b", "hello".to_string(), &provider);

        assert_eq!(
            request.options.map(|options| options.num_ctx),
            Some(DEFAULT_OLLAMA_NUM_CTX)
        );
    }

    #[test]
    fn omits_num_ctx_for_non_ollama_requests() {
        let provider = build_provider("anamedi", "https://app.anamedi.com");
        let request = build_chat_completion_request("model", "hello".to_string(), &provider);

        assert!(request.options.is_none());
    }
}

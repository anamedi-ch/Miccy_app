//! Managed local post-processing using a downloaded `llama-server` binary (llama.cpp)
//! and curated GGUF models — no manual localhost setup for end users.

use futures_util::StreamExt;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use zip::ZipArchive;

use crate::settings::LocalLlmPerformancePreset;

/// Post-process provider id for on-device summarization (settings + UI).
pub const PROVIDER_ID: &str = "local_private";

pub const TIER_FAST: &str = "local_fast";
pub const TIER_QUALITY: &str = "local_quality";
pub const TIER_APERTUS: &str = "local_apertus";

/// Frontend listens for toasts / diagnostics when local post-processing fails.
pub const LOCAL_PRIVATE_ERROR_EVENT: &str = "local-private-llm-error";

const LLAMA_RELEASE: &str = "b8748";

/// Extra headroom beyond catalog size when checking free disk space (bytes).
const DOWNLOAD_DISK_HEADROOM: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Type)]
pub struct LocalPrivateLlmErrorPayload {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn emit_local_private_error(app: &AppHandle, code: &str, detail: Option<String>) {
    let _ = app.emit(
        LOCAL_PRIVATE_ERROR_EVENT,
        &LocalPrivateLlmErrorPayload {
            code: code.to_string(),
            detail,
        },
    );
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalLlmModelInfo {
    pub tier_id: String,
    pub filename: String,
    pub size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalLlmDownloadProgress {
    pub tier_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalLlmRuntimeStatus {
    pub ready: bool,
    pub platform_supported: bool,
}

struct CatalogEntry {
    tier_id: &'static str,
    filename: &'static str,
    url: &'static str,
    size_mb: u64,
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        tier_id: TIER_FAST,
        filename: "Qwen3.5-4B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf",
        size_mb: 2615,
    },
    CatalogEntry {
        tier_id: TIER_QUALITY,
        filename: "Qwen3.5-9B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf",
        size_mb: 5420,
    },
    CatalogEntry {
        tier_id: TIER_APERTUS,
        filename: "Apertus-8B-Instruct-2509-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Apertus-8B-Instruct-2509-GGUF/resolve/main/Apertus-8B-Instruct-2509-Q4_K_M.gguf",
        size_mb: 4825,
    },
];

#[allow(dead_code)] // Zip used on Windows builds only
enum RuntimeArchive {
    TarGz { url: &'static str },
    Zip { url: &'static str },
}

struct RuntimeTarget {
    key: &'static str,
    archive: RuntimeArchive,
    server_relative: &'static str,
}

fn runtime_target() -> Result<RuntimeTarget, String> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok(RuntimeTarget {
            key: "macos-arm64",
            archive: RuntimeArchive::TarGz {
                url: "https://github.com/ggml-org/llama.cpp/releases/download/b8748/llama-b8748-bin-macos-arm64.tar.gz",
            },
            server_relative: "llama-b8748/llama-server",
        });
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok(RuntimeTarget {
            key: "macos-x64",
            archive: RuntimeArchive::TarGz {
                url: "https://github.com/ggml-org/llama.cpp/releases/download/b8748/llama-b8748-bin-macos-x64.tar.gz",
            },
            server_relative: "llama-b8748/llama-server",
        });
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok(RuntimeTarget {
            key: "linux-x64",
            archive: RuntimeArchive::TarGz {
                url: "https://github.com/ggml-org/llama.cpp/releases/download/b8748/llama-b8748-bin-ubuntu-x64.tar.gz",
            },
            server_relative: "llama-b8748/llama-server",
        });
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok(RuntimeTarget {
            key: "win-cpu-x64",
            archive: RuntimeArchive::Zip {
                url: "https://github.com/ggml-org/llama.cpp/releases/download/b8748/llama-b8748-bin-win-cpu-x64.zip",
            },
            server_relative: "llama-server.exe",
        });
    }
    #[allow(unreachable_code)]
    Err("Local summarization is not supported on this platform.".to_string())
}

fn app_llama_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("llama"))
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app_llama_root(app)?;
    Ok(root.join("models"))
}

fn runtime_version_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_llama_root(app)?.join(format!("runtime-{}.txt", LLAMA_RELEASE)))
}

fn catalog_for_tier(tier_id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.tier_id == tier_id)
}

pub fn list_catalog() -> Vec<LocalLlmModelInfo> {
    CATALOG
        .iter()
        .map(|e| LocalLlmModelInfo {
            tier_id: e.tier_id.to_string(),
            filename: e.filename.to_string(),
            size_mb: e.size_mb,
        })
        .collect()
}

pub fn resolve_model_path(app: &AppHandle, tier_id: &str) -> Result<PathBuf, String> {
    let entry = catalog_for_tier(tier_id)
        .ok_or_else(|| format!("Unknown local model tier: {}", tier_id))?;
    Ok(models_dir(app)?.join(entry.filename))
}

pub fn is_model_file_present(app: &AppHandle, tier_id: &str) -> Result<bool, String> {
    let path = resolve_model_path(app, tier_id)?;
    Ok(path.exists() && path.is_file())
}

fn required_download_bytes(entry: &CatalogEntry) -> u64 {
    entry
        .size_mb
        .saturating_mul(1024 * 1024)
        .saturating_add(DOWNLOAD_DISK_HEADROOM)
}

fn ensure_disk_space_for_download(
    app: &AppHandle,
    dest_dir: &Path,
    entry: &CatalogEntry,
) -> Result<(), String> {
    let required = required_download_bytes(entry);
    let space = fs4::available_space(dest_dir).map_err(|e| e.to_string())?;
    if space < required {
        let mb_need = required / (1024 * 1024);
        let mb_have = space / (1024 * 1024);
        let msg = format!(
            "Need about {} MiB free (including buffer); only about {} MiB available.",
            mb_need, mb_have
        );
        emit_local_private_error(app, "insufficient_disk_space", Some(msg.clone()));
        return Err(msg);
    }
    Ok(())
}

fn stderr_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_llama_root(app)?.join("llama-server-stderr.log"))
}

fn read_stderr_tail(app: &AppHandle, max_bytes: usize) -> Option<String> {
    let path = stderr_log_path(app).ok()?;
    let data = fs::read(&path).ok()?;
    let slice = if data.len() > max_bytes {
        &data[data.len() - max_bytes..]
    } else {
        &data[..]
    };
    let s = String::from_utf8_lossy(slice).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn gpu_layers_for_preset(preset: LocalLlmPerformancePreset) -> u32 {
    match preset {
        LocalLlmPerformancePreset::Low => 0,
        LocalLlmPerformancePreset::Default | LocalLlmPerformancePreset::High => 99,
    }
}

async fn fetch_first_model_id(base_v1: &str) -> Result<String, String> {
    let url = format!("{}/models", base_v1.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach local server: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Local server returned {}", response.status()));
    }
    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Invalid JSON from local server: {}", e))?;
    let id = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|o| o.get("id"))
        .and_then(|i| i.as_str())
        .map(String::from)
        .ok_or_else(|| "Could not read model id from local server".to_string())?;
    Ok(id)
}

async fn wait_for_server(app: &AppHandle, base_v1: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let url = format!("{}/models", base_v1.trim_end_matches('/'));
    for attempt in 0..90 {
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => return Ok(()),
            Ok(r) => debug!("local llama: GET /v1/models -> {}", r.status()),
            Err(e) => debug!("local llama: waiting for server ({})", e),
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if attempt == 89 {
            let mut msg =
                "Summarization could not start. Try again or pick a smaller model.".to_string();
            if let Some(tail) = read_stderr_tail(app, 4000) {
                msg.push_str("\n");
                msg.push_str(&tail);
            }
            emit_local_private_error(app, "server_start_failed", Some(msg.clone()));
            return Err(msg);
        }
    }
    Ok(())
}

fn pick_free_port() -> Result<u16, String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("No free TCP port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Socket: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| format!("Open archive: {}", e))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    fs::create_dir_all(dest).map_err(|e| format!("Create dir: {}", e))?;
    archive
        .unpack(dest)
        .map_err(|e| format!("Extract failed: {}", e))?;
    Ok(())
}

fn extract_zip_safe(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| format!("Open zip: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Read zip: {}", e))?;
    fs::create_dir_all(dest).map_err(|e| format!("Create dir: {}", e))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry: {}", e))?;
        let outpath = match file.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue,
        };
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| format!("mkdir: {}", e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| format!("create: {}", e))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| format!("write: {}", e))?;
        }
    }
    Ok(())
}

pub struct LocalLlmCoordinator {
    inner: StdMutex<LocalLlmInner>,
    idle_shutdown: StdMutex<Option<tokio::task::JoinHandle<()>>>,
}

struct LocalLlmInner {
    child: Option<Child>,
    port: u16,
    model_path: Option<PathBuf>,
    preset: Option<LocalLlmPerformancePreset>,
    ctx: Option<u32>,
    model_api_id: Option<String>,
}

impl Default for LocalLlmInner {
    fn default() -> Self {
        Self {
            child: None,
            port: 0,
            model_path: None,
            preset: None,
            ctx: None,
            model_api_id: None,
        }
    }
}

impl LocalLlmCoordinator {
    pub fn new() -> Self {
        Self {
            inner: StdMutex::new(LocalLlmInner::default()),
            idle_shutdown: StdMutex::new(None),
        }
    }

    fn abort_idle_shutdown(&self) {
        if let Ok(mut g) = self.idle_shutdown.lock() {
            if let Some(h) = g.take() {
                h.abort();
            }
        }
    }

    /// After a local post-process request finishes, stop `llama-server` after `minutes`
    /// of inactivity to free RAM. `minutes == 0` disables this behavior.
    pub fn schedule_idle_shutdown_after_use(&self, app: &AppHandle, minutes: u32) {
        self.abort_idle_shutdown();
        if minutes == 0 {
            return;
        }
        let app = app.clone();
        let secs = u64::from(minutes).saturating_mul(60);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            stop_server(&app);
        });
        if let Ok(mut g) = self.idle_shutdown.lock() {
            *g = Some(handle);
        }
    }

    pub async fn runtime_status(&self, app: &AppHandle) -> LocalLlmRuntimeStatus {
        let platform_supported = runtime_target().is_ok();
        if !platform_supported {
            return LocalLlmRuntimeStatus {
                ready: false,
                platform_supported: false,
            };
        }
        let ready = match server_binary_path(app) {
            Ok(p) => p.exists(),
            Err(_) => false,
        };
        LocalLlmRuntimeStatus {
            ready,
            platform_supported: true,
        }
    }

    pub async fn ensure_runtime_binary(&self, app: &AppHandle) -> Result<(), String> {
        let target = runtime_target()?;
        if server_binary_path(app)?.exists() {
            let _ = fs::write(runtime_version_path(app)?, LLAMA_RELEASE);
            return Ok(());
        }
        let root = app_llama_root(app)?;
        fs::create_dir_all(&root).map_err(|e| format!("Create llama dir: {}", e))?;
        let staging = root.join("downloads");
        fs::create_dir_all(&staging).map_err(|e| format!("Create staging: {}", e))?;
        let archive_name = match &target.archive {
            RuntimeArchive::TarGz { .. } => format!("runtime-{}.tar.gz", target.key),
            RuntimeArchive::Zip { .. } => format!("runtime-{}.zip", target.key),
        };
        let archive_path = staging.join(&archive_name);
        let url = match &target.archive {
            RuntimeArchive::TarGz { url } | RuntimeArchive::Zip { url } => url,
        };
        info!("Downloading llama.cpp runtime from {}", url);
        let client = reqwest::Client::new();
        let response = client
            .get(*url)
            .send()
            .await
            .map_err(|e| format!("Download runtime failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!(
                "Download runtime failed: HTTP {}",
                response.status()
            ));
        }
        let mut file = File::create(&archive_path).map_err(|e| format!("Create file: {}", e))?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Read download: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("Write runtime: {}", e))?;
        }
        file.flush().map_err(|e| format!("Flush: {}", e))?;
        drop(file);
        let extract_to = root.join("runtime").join(target.key);
        if extract_to.exists() {
            let _ = fs::remove_dir_all(&extract_to);
        }
        fs::create_dir_all(&extract_to).map_err(|e| format!("Extract dir: {}", e))?;
        match &target.archive {
            RuntimeArchive::TarGz { .. } => extract_tar_gz(&archive_path, &extract_to)?,
            RuntimeArchive::Zip { .. } => extract_zip_safe(&archive_path, &extract_to)?,
        }
        let _ = fs::remove_file(&archive_path);
        let exe = extract_to.join(target.server_relative);
        if !exe.exists() {
            return Err(
                "Summarization helper installed incorrectly. Try again or reinstall the app."
                    .to_string(),
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exe)
                .map_err(|e| format!("stat: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe, perms).map_err(|e| format!("chmod: {}", e))?;
        }
        fs::write(runtime_version_path(app)?, LLAMA_RELEASE).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn download_tier(&self, app: AppHandle, tier_id: String) -> Result<(), String> {
        let entry = match catalog_for_tier(&tier_id) {
            Some(e) => e,
            None => {
                let msg = format!("Unknown tier: {}", tier_id);
                emit_local_private_error(&app, "download_failed", Some(msg.clone()));
                return Err(msg);
            }
        };
        let dest = resolve_model_path(&app, &tier_id)?;
        if dest.exists() {
            return Ok(());
        }
        let parent = dest.parent().ok_or("model parent")?;
        fs::create_dir_all(parent).map_err(|e| format!("Create models dir: {}", e))?;
        if let Err(e) = ensure_disk_space_for_download(&app, parent, entry) {
            return Err(e);
        }
        let partial = dest.with_extension("gguf.partial");
        let client = reqwest::Client::new();
        let mut resume_from = if partial.exists() {
            partial.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let mut request = client.get(entry.url);
        if resume_from > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from));
        }
        let mut response = request.send().await.map_err(|e| {
            let msg = format!("Download failed: {}", e);
            emit_local_private_error(&app, "download_failed", Some(msg.clone()));
            msg
        })?;
        if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
            let _ = fs::remove_file(&partial);
            resume_from = 0;
            response = client.get(entry.url).send().await.map_err(|e| {
                let msg = format!("Download failed: {}", e);
                emit_local_private_error(&app, "download_failed", Some(msg.clone()));
                msg
            })?;
        }
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            let msg = format!("Download failed: HTTP {}", response.status());
            emit_local_private_error(&app, "download_failed", Some(msg.clone()));
            return Err(msg);
        }
        let total_size = if resume_from > 0 {
            resume_from + response.content_length().unwrap_or(0)
        } else {
            response.content_length().unwrap_or(0)
        };
        let mut downloaded = resume_from;
        let mut file = if resume_from > 0 {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial)
                .map_err(|e| e.to_string())?
        } else {
            File::create(&partial).map_err(|e| e.to_string())?
        };
        let mut stream = response.bytes_stream();
        let _ = app.emit(
            "local-llm-download-progress",
            &LocalLlmDownloadProgress {
                tier_id: tier_id.clone(),
                downloaded,
                total: total_size,
                percentage: if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                },
            },
        );
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                let msg = format!("Stream: {}", e);
                emit_local_private_error(&app, "download_failed", Some(msg.clone()));
                msg
            })?;
            file.write_all(&chunk).map_err(|e| {
                let msg = e.to_string();
                emit_local_private_error(&app, "download_failed", Some(msg.clone()));
                msg
            })?;
            downloaded += chunk.len() as u64;
            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "local-llm-download-progress",
                &LocalLlmDownloadProgress {
                    tier_id: tier_id.clone(),
                    downloaded,
                    total: total_size,
                    percentage,
                },
            );
        }
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&partial, &dest).map_err(|e| {
            let msg = format!("Finalize model: {}", e);
            emit_local_private_error(&app, "download_failed", Some(msg.clone()));
            msg
        })?;
        Ok(())
    }

    pub async fn ensure_server_for_post_process(
        &self,
        app: &AppHandle,
        model_path: &Path,
        preset: LocalLlmPerformancePreset,
        ctx: u32,
    ) -> Result<(PostProcessProviderForLocal, String), String> {
        self.abort_idle_shutdown();
        self.ensure_runtime_binary(app).await?;
        let need_restart = {
            let guard = self
                .inner
                .lock()
                .map_err(|_| "Local server lock poisoned".to_string())?;
            match (&guard.child, &guard.model_path, guard.preset, guard.ctx) {
                (Some(_), Some(p), Some(pr), Some(c)) => {
                    p != model_path || pr != preset || c != ctx || guard.port == 0
                }
                _ => true,
            }
        };
        if need_restart {
            {
                let mut guard = self
                    .inner
                    .lock()
                    .map_err(|_| "Local server lock poisoned".to_string())?;
                if let Some(mut c) = guard.child.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                guard.model_api_id = None;
                let port = pick_free_port()?;
                let work_dir = server_binary_path(app)?
                    .parent()
                    .ok_or("server path")?
                    .to_path_buf();
                let exe = server_binary_path(app)?;
                let ngl = gpu_layers_for_preset(preset);
                let stderr_file = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(stderr_log_path(app)?)
                    .map_err(|e| format!("stderr log: {}", e))?;
                let mut cmd = Command::new(&exe);
                cmd.current_dir(&work_dir)
                    .arg("-m")
                    .arg(model_path)
                    .arg("-c")
                    .arg(ctx.to_string())
                    .arg("--host")
                    .arg("127.0.0.1")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("-ngl")
                    .arg(ngl.to_string())
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::from(stderr_file));
                let child = cmd.spawn().map_err(|e| {
                    let mut msg = format!(
                        "Summarization could not start. Try again or pick a smaller model. ({})",
                        e
                    );
                    if let Some(tail) = read_stderr_tail(app, 4000) {
                        msg.push_str("\n");
                        msg.push_str(&tail);
                    }
                    emit_local_private_error(app, "server_start_failed", Some(msg.clone()));
                    msg
                })?;
                guard.child = Some(child);
                guard.port = port;
                guard.model_path = Some(model_path.to_path_buf());
                guard.preset = Some(preset);
                guard.ctx = Some(ctx);
            }
            let base_v1 = {
                let guard = self
                    .inner
                    .lock()
                    .map_err(|_| "Local server lock poisoned".to_string())?;
                format!("http://127.0.0.1:{}/v1", guard.port)
            };
            wait_for_server(app, &base_v1).await?;
            let model_id = fetch_first_model_id(&base_v1).await.map_err(|mut e| {
                if let Some(tail) = read_stderr_tail(app, 4000) {
                    e.push_str("\n");
                    e.push_str(&tail);
                }
                emit_local_private_error(app, "server_start_failed", Some(e.clone()));
                e
            })?;
            {
                let mut guard = self
                    .inner
                    .lock()
                    .map_err(|_| "Local server lock poisoned".to_string())?;
                guard.model_api_id = Some(model_id.clone());
            }
            return Ok((PostProcessProviderForLocal { base_url: base_v1 }, model_id));
        }
        let (port, model_id) = {
            let guard = self
                .inner
                .lock()
                .map_err(|_| "Local server lock poisoned".to_string())?;
            let model_id = guard
                .model_api_id
                .clone()
                .ok_or_else(|| "Local server state lost".to_string())?;
            (guard.port, model_id)
        };
        let base_v1 = format!("http://127.0.0.1:{}/v1", port);
        Ok((PostProcessProviderForLocal { base_url: base_v1 }, model_id))
    }

    pub fn stop(&self) {
        self.abort_idle_shutdown();
        if let Ok(mut g) = self.inner.lock() {
            if let Some(mut c) = g.child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            g.model_api_id = None;
            g.model_path = None;
            g.preset = None;
            g.ctx = None;
            g.port = 0;
        }
    }
}

pub struct PostProcessProviderForLocal {
    pub base_url: String,
}

fn server_binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    let target = runtime_target()?;
    let root = app_llama_root(app)?;
    Ok(root
        .join("runtime")
        .join(target.key)
        .join(target.server_relative))
}

pub fn stop_server(app: &AppHandle) {
    if let Some(coord) = app.try_state::<Arc<LocalLlmCoordinator>>() {
        coord.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::LocalLlmPerformancePreset;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Cursor, Write};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tar::Builder;
    use tar::Header;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("anamedi_local_llm_{}_{}", name, nanos))
    }

    #[test]
    fn provider_and_tier_constants_match_settings_contract() {
        assert_eq!(PROVIDER_ID, "local_private");
        assert_eq!(TIER_FAST, "local_fast");
        assert_eq!(TIER_QUALITY, "local_quality");
        assert_eq!(TIER_APERTUS, "local_apertus");
    }

    #[test]
    fn list_catalog_has_three_tiers_with_expected_filenames() {
        let catalog = list_catalog();
        assert_eq!(catalog.len(), 3);
        assert!(catalog.iter().any(|e| e.tier_id == TIER_FAST));
        assert!(catalog.iter().any(|e| e.tier_id == TIER_QUALITY));
        assert!(catalog.iter().any(|e| e.tier_id == TIER_APERTUS));
        let fast = catalog.iter().find(|e| e.tier_id == TIER_FAST).unwrap();
        assert_eq!(fast.filename, "Qwen3.5-4B-Q4_K_M.gguf");
        assert!(fast.size_mb > 0);
        let apertus = catalog.iter().find(|e| e.tier_id == TIER_APERTUS).unwrap();
        assert!(apertus.filename.contains("Apertus"));
        assert!(apertus.size_mb > 1000);
    }

    #[test]
    fn catalog_for_tier_resolves_known_tiers() {
        let fast = catalog_for_tier(TIER_FAST).expect("fast tier");
        assert!(fast.url.contains("Qwen3.5-4B"));
        assert!(fast.url.contains("unsloth"));
        let quality = catalog_for_tier(TIER_QUALITY).expect("quality tier");
        assert!(quality.url.contains("Qwen3.5-9B"));
        let apertus = catalog_for_tier(TIER_APERTUS).expect("apertus tier");
        assert!(apertus.filename.starts_with("Apertus"));
        assert!(apertus.url.contains("unsloth"));
        assert!(catalog_for_tier("unknown_tier").is_none());
    }

    #[test]
    fn gpu_layers_for_preset_maps_cpu_and_gpu_modes() {
        assert_eq!(gpu_layers_for_preset(LocalLlmPerformancePreset::Low), 0);
        assert_eq!(
            gpu_layers_for_preset(LocalLlmPerformancePreset::Default),
            99
        );
        assert_eq!(gpu_layers_for_preset(LocalLlmPerformancePreset::High), 99);
    }

    #[test]
    fn pick_free_port_returns_ephemeral_port() {
        let a = pick_free_port().expect("port a");
        let b = pick_free_port().expect("port b");
        assert!(a > 0);
        assert!(b > 0);
        assert_ne!(a, b);
    }

    #[test]
    fn runtime_target_supported_on_ci_desktop_triples() {
        #[cfg(any(
            all(
                target_os = "macos",
                any(target_arch = "aarch64", target_arch = "x86_64")
            ),
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86_64"),
        ))]
        {
            let t = runtime_target().expect("supported platform");
            assert!(!t.key.is_empty());
            assert!(t.server_relative.contains("llama-server"));
        }
    }

    #[test]
    fn extract_zip_safe_writes_regular_file() {
        let root = unique_test_dir("zip");
        fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("sample.zip");
        let dest = root.join("out");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();
            zip.start_file("nested/hello.txt", opts).unwrap();
            zip.write_all(b"zip-content").unwrap();
            zip.finish().unwrap();
        }
        extract_zip_safe(&zip_path, &dest).expect("extract zip");
        let text = fs::read_to_string(dest.join("nested/hello.txt")).unwrap();
        assert_eq!(text, "zip-content");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn extract_tar_gz_unpacks_single_file() {
        let root = unique_test_dir("tar");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.gz");
        {
            let file = File::create(&archive_path).unwrap();
            let enc = GzEncoder::new(file, Compression::default());
            let mut builder = Builder::new(enc);
            let mut header = Header::new_gnu();
            header.set_path("notes.txt").unwrap();
            header.set_size(4);
            header.set_cksum();
            builder.append(&header, Cursor::new("test")).unwrap();
            builder.finish().unwrap();
        }
        let dest = root.join("extracted");
        extract_tar_gz(&archive_path, &dest).expect("extract tar.gz");
        let text = fs::read_to_string(dest.join("notes.txt")).unwrap();
        assert_eq!(text, "test");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn local_llm_download_progress_serializes_for_events() {
        let p = LocalLlmDownloadProgress {
            tier_id: TIER_FAST.to_string(),
            downloaded: 50,
            total: 100,
            percentage: 50.0,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("local_fast"));
        assert!(json.contains("50"));
    }
}

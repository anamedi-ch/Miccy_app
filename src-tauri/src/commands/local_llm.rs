use crate::managers::local_llm::{
    self, LocalLlmCoordinator, LocalLlmModelInfo, LocalLlmRuntimeStatus,
};
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub async fn get_local_llm_catalog(_app: AppHandle) -> Result<Vec<LocalLlmModelInfo>, String> {
    Ok(local_llm::list_catalog())
}

#[tauri::command]
#[specta::specta]
pub async fn get_local_llm_runtime_status(
    app: AppHandle,
    coordinator: tauri::State<'_, Arc<LocalLlmCoordinator>>,
) -> Result<LocalLlmRuntimeStatus, String> {
    Ok(coordinator.runtime_status(&app).await)
}

#[tauri::command]
#[specta::specta]
pub async fn is_local_llm_model_downloaded(
    app: AppHandle,
    tier_id: String,
) -> Result<bool, String> {
    local_llm::is_model_file_present(&app, &tier_id)
}

#[tauri::command]
#[specta::specta]
pub async fn download_local_llm_model(
    app: AppHandle,
    coordinator: tauri::State<'_, Arc<LocalLlmCoordinator>>,
    tier_id: String,
) -> Result<(), String> {
    coordinator.download_tier(app, tier_id).await
}

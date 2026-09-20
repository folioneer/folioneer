//! Logging infrastructure and frontend log bridge.

/// Context identifier for frontend-initiated log messages.
pub const FRONTEND: &str = "frontend";

/// Context identifier for backend-originated log messages.
pub const BACKEND: &str = "backend";

/// Tauri command allowing the frontend to emit structured log entries
/// into the backend tracing system (visible in app logs and collect-logs output).
#[tauri::command]
#[specta::specta]
pub fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "trace" => tracing::trace!(target: FRONTEND, "{}", message),
        "debug" => tracing::debug!(target: FRONTEND, "{}", message),
        "info" => tracing::info!(target: FRONTEND, "{}", message),
        "warn" => tracing::warn!(target: FRONTEND, "{}", message),
        "error" => tracing::error!(target: FRONTEND, "{}", message),
        _ => tracing::info!(target: FRONTEND, "{}", message),
    }
}

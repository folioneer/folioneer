//! Update checker service — detects, downloads, and installs application updates.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, Url};
use tauri_plugin_updater::{Updater, UpdaterExt};

use super::error::UpdateError;
use crate::core::BACKEND;

/// Information about an available application update.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct UpdateInfo {
    /// Semantic version string of the available update (e.g. "1.2.3").
    pub version: String,
}

/// The channel a build's updates come from (ADR-020), managed on the app handle by the
/// composition root. No endpoint means the endpoint of `tauri.conf.json`; the headers
/// travel with the check and with the download.
///
/// Credentials go in the `Authorization` header and nowhere else: it is the header the
/// HTTP client drops when a redirect leaves the host (release downloads do redirect),
/// whereas any other header follows the redirect, and an endpoint URL is logged when a
/// check fails. The type has no `Debug` so a channel cannot be logged by accident.
#[derive(Clone, Default)]
pub struct UpdateChannel {
    /// Endpoints that replace the configured one, tried in order.
    pub endpoints: Vec<Url>,
    /// Request headers as (name, value) — never logged.
    pub headers: Vec<(String, String)>,
}

/// Shared state for the update lifecycle, managed across Tauri commands.
///
/// Tracks whether a download is in progress (R10) and stores downloaded bytes
/// between the download command and the install command.
#[derive(Debug, Default)]
pub struct UpdateState {
    /// True while a download is in progress — prevents concurrent downloads (R10).
    pub is_downloading: AtomicBool,
    /// Downloaded installer bytes stored after a successful download (R9).
    downloaded_bytes: Mutex<Option<Vec<u8>>>,
}

impl UpdateState {
    /// Creates a new, empty update state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores downloaded bytes for later installation.
    pub fn set_bytes(&self, bytes: Vec<u8>) {
        if let Ok(mut guard) = self.downloaded_bytes.lock() {
            *guard = Some(bytes);
        }
    }

    /// Takes the downloaded bytes, clearing the stored value.
    pub fn take_bytes(&self) -> Option<Vec<u8>> {
        self.downloaded_bytes.lock().ok()?.take()
    }
}

/// The channel the composition root manages, or the configured one when none is.
fn managed_channel<R: Runtime>(app_handle: &AppHandle<R>) -> UpdateChannel {
    app_handle
        .try_state::<UpdateChannel>()
        .map(|channel| channel.inner().clone())
        .unwrap_or_default()
}

/// The updater of a channel: its endpoints replace the configured one, its headers
/// join every request the updater sends.
fn channel_updater<R: Runtime>(
    app_handle: &AppHandle<R>,
    channel: &UpdateChannel,
) -> tauri_plugin_updater::Result<Updater> {
    let mut builder = app_handle.updater_builder();
    if !channel.endpoints.is_empty() {
        builder = builder.endpoints(channel.endpoints.clone())?;
    }
    for (name, value) in &channel.headers {
        builder = builder.header(name.as_str(), value.as_str())?;
    }
    builder.build()
}

/// Whether the update server refuses this build's access (UPD-028). The updater plugin
/// hides the response status, so a failed check on a channel that sends headers is
/// followed by one request of our own; 401 and 403 are a refusal. A channel without
/// headers is never probed — an anonymous failure stays silent (UPD-021).
async fn access_refused(channel: &UpdateChannel) -> bool {
    match channel.endpoints.first() {
        Some(endpoint) => access_refused_at(endpoint, channel).await,
        None => false,
    }
}

/// [`access_refused`] for one address of the channel: its endpoint after a failed
/// check, the installer's address after a failed download.
async fn access_refused_at(address: &Url, channel: &UpdateChannel) -> bool {
    if channel.headers.is_empty() {
        return false;
    }
    // The answer of the endpoint itself is the one that counts, and no header of the
    // channel may travel to another host: redirects are not followed.
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(target: BACKEND, error = %error, "Update access probe could not start");
            return false;
        }
    };
    let mut request = client.get(address.clone());
    for (name, value) in &channel.headers {
        request = request.header(name, value);
    }
    match request.send().await {
        Ok(response) => matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ),
        Err(error) => {
            tracing::warn!(target: BACKEND, error = %error.without_url(), "Update access probe failed");
            false
        }
    }
}

/// `AccessRefused` when the server refuses this build's access, `OperationFailed` otherwise.
fn failure_of(refused: bool) -> UpdateError {
    if refused {
        tracing::warn!(target: BACKEND, "Update server refused this build's access (UPD-028)");
        UpdateError::AccessRefused
    } else {
        UpdateError::OperationFailed
    }
}

/// Checks whether a new application version is available.
///
/// Returns `Ok(None)` silently on network or server errors (UPD-021), logging them for
/// diagnostics (UPD-022) — except a refused access, which is returned and announced on
/// `"update:error"` (UPD-028). Emits `"update:available"` on the app handle if an update
/// is found, so that all listeners (banner, manual check) react consistently.
pub async fn check<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<Option<UpdateInfo>, UpdateError> {
    let channel = managed_channel(app_handle);
    let updater = match channel_updater(app_handle, &channel) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(target: BACKEND, error = %e, "Failed to initialize updater (R22)");
            return Ok(None);
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            tracing::info!(target: BACKEND, version = %version, "Update available");
            let info = UpdateInfo { version };
            let _ = app_handle.emit("update:available", &info);
            Ok(Some(info))
        }
        Ok(None) => {
            tracing::info!(target: BACKEND, "Application is up to date");
            Ok(None)
        }
        Err(e) => {
            tracing::warn!(target: BACKEND, error = %e, "Update check failed (R21, R22)");
            match failure_of(access_refused(&channel).await) {
                UpdateError::AccessRefused => {
                    let _ = app_handle.emit("update:error", &UpdateError::AccessRefused);
                    Err(UpdateError::AccessRefused)
                }
                _ => Ok(None),
            }
        }
    }
}

/// Downloads the available update in the background, emitting progress events (R8).
///
/// Does nothing if a download is already in progress (R10).
/// Emits `"update:progress"` (percent 0–100) during download,
/// `"update:complete"` on success, or `"update:error"` on failure (R23).
/// Checksum verification is performed by the Tauri updater plugin (R9).
pub async fn download(app_handle: AppHandle, state: Arc<UpdateState>) -> Result<(), UpdateError> {
    // R10 — prevent concurrent downloads
    if state
        .is_downloading
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!(target: BACKEND, "Download already in progress — ignoring (R10)");
        return Ok(());
    }

    let result = do_download(&app_handle, &state).await;
    state.is_downloading.store(false, Ordering::SeqCst);

    if let Err(ref error) = result {
        // The underlying cause is logged inside do_download; emit the typed error
        // (never the raw cause string) so the renderer can localise it (R23).
        let _ = app_handle.emit("update:error", error);
    }

    result
}

async fn do_download(app_handle: &AppHandle, state: &UpdateState) -> Result<(), UpdateError> {
    let channel = managed_channel(app_handle);
    let updater = channel_updater(app_handle, &channel).map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Failed to initialize updater (R23)");
        UpdateError::OperationFailed
    })?;

    let update = match updater.check().await {
        Ok(update) => update,
        Err(e) => {
            tracing::error!(target: BACKEND, error = %e, "Failed to check for update during download (R23)");
            return Err(failure_of(access_refused(&channel).await));
        }
    }
        .ok_or_else(|| {
            tracing::error!(target: BACKEND, "No update available to download (R23)");
            UpdateError::OperationFailed
        })?;

    let downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let ah = app_handle.clone();

    // R8 — emit progress events; checksum is verified by the plugin (R9)
    let installer_address = update.download_url.clone();
    let bytes = match update
        .download(
            move |chunk, total| {
                let current = downloaded.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let percent = total
                    .and_then(|t| (current * 100).checked_div(t))
                    .map(|p| p.min(100))
                    .unwrap_or(0);
                let _ = ah.emit("update:progress", percent);
            },
            || {},
        )
        .await
    {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!(target: BACKEND, error = %e, "Download or checksum verification failed (R9, R23)");
            return Err(failure_of(
                access_refused_at(&installer_address, &channel).await,
            ));
        }
    };

    // Store bytes BEFORE emitting complete — prevents install racing (R11)
    state.set_bytes(bytes);
    let _ = app_handle.emit("update:complete", ());
    tracing::info!(target: BACKEND, "Update downloaded and checksum verified (R9)");
    Ok(())
}

/// Installs the previously downloaded update and restarts the application (R13).
///
/// Re-checks for the update to obtain a fresh handle for the install call.
/// Requires that `download` has been called successfully beforehand.
pub async fn install(app_handle: AppHandle, state: Arc<UpdateState>) -> Result<(), UpdateError> {
    let bytes = state.take_bytes().ok_or(UpdateError::NoDownloadedUpdate)?;

    let updater = channel_updater(&app_handle, &managed_channel(&app_handle)).map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Failed to initialize updater for install");
        UpdateError::OperationFailed
    })?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            tracing::error!(target: BACKEND, error = %e, "Failed to get update for installation");
            UpdateError::OperationFailed
        })?
        .ok_or_else(|| {
            tracing::error!(target: BACKEND, "No update found for installation");
            UpdateError::OperationFailed
        })?;

    update.install(bytes).map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Installation failed");
        UpdateError::OperationFailed
    })?;
    tracing::info!(target: BACKEND, "Update installed — restarting application (R13)");
    app_handle.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A server that answers its one request with `status` and hands the request text back.
    async fn serve_once(status: &'static str) -> (Url, tokio::task::JoinHandle<String>) {
        serve(status, 1).await
    }

    /// A server that answers `count` requests with `status` — a refused check costs two,
    /// the updater's own and the probe that classifies it — and hands the first request
    /// text back.
    async fn serve(status: &'static str, count: usize) -> (Url, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let served = tokio::spawn(async move {
            let mut first = String::new();
            for answered in 0..count {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let mut buffer = vec![0; 8192];
                let read = stream.read(&mut buffer).await.expect("read");
                let response =
                    format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
                stream.write_all(response.as_bytes()).await.expect("write");
                if answered == 0 {
                    first = String::from_utf8_lossy(&buffer[..read]).to_lowercase();
                }
            }
            first
        });
        let endpoint = format!("http://{address}/latest.json")
            .parse()
            .expect("url");
        (endpoint, served)
    }

    fn channel(endpoint: Url, headers: &[(&str, &str)]) -> UpdateChannel {
        UpdateChannel {
            endpoints: vec![endpoint],
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
        }
    }

    /// A mock application carrying the updater plugin and, optionally, a channel.
    fn app_with(channel: Option<UpdateChannel>) -> tauri::App<tauri::test::MockRuntime> {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({
                "pubkey": "unused",
                "endpoints": ["https://configured.invalid/latest.json"]
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .expect("mock application");
        if let Some(channel) = channel {
            app.handle().manage(channel);
        }
        app
    }

    // #037 — a build that manages no channel updates from the configured endpoint.
    #[test]
    fn a_build_without_a_managed_channel_reads_the_configured_one() {
        let app = app_with(None);

        let channel = managed_channel(app.handle());

        assert!(channel.endpoints.is_empty());
        assert!(channel.headers.is_empty());
    }

    // #037 — the channel the composition root manages is the one every request uses.
    #[tokio::test]
    async fn the_managed_channel_is_the_one_the_check_uses() {
        let (endpoint, served) = serve_once("204 No Content").await;
        let app = app_with(Some(channel(
            endpoint,
            &[("authorization", "Bearer channel-token")],
        )));

        let update = check(app.handle()).await.expect("check");

        assert!(update.is_none());
        assert!(served
            .await
            .expect("served")
            .contains("authorization: bearer channel-token"));
    }

    // UPD-028 — a refused check answers the refusal instead of "no update".
    #[tokio::test]
    async fn a_refused_check_returns_the_refusal() {
        let (endpoint, _served) = serve("401 Unauthorized", 2).await;
        let app = app_with(Some(channel(
            endpoint,
            &[("authorization", "Bearer expired")],
        )));

        let outcome = check(app.handle()).await;

        assert!(matches!(outcome, Err(UpdateError::AccessRefused)));
    }

    // UPD-021 — a failure on a channel that sends nothing stays silent.
    #[tokio::test]
    async fn an_anonymous_failure_stays_silent() {
        let (endpoint, _served) = serve_once("500 Internal Server Error").await;
        let app = app_with(Some(UpdateChannel {
            endpoints: vec![endpoint],
            headers: Vec::new(),
        }));

        let outcome = check(app.handle()).await;

        assert!(matches!(outcome, Ok(None)));
    }

    // #037 — the endpoint and the headers of the channel are the ones the updater's own
    // check request goes to and carries.
    #[tokio::test]
    async fn a_replaced_endpoint_and_header_reach_the_update_request() {
        let (endpoint, served) = serve_once("204 No Content").await;
        let app = app_with(None);

        let updater = channel_updater(
            app.handle(),
            &channel(endpoint, &[("authorization", "Bearer channel-token")]),
        )
        .expect("updater");
        let update = updater.check().await.expect("check");

        assert!(update.is_none());
        let request = served.await.expect("served");
        assert!(request.starts_with("get /latest.json "));
        assert!(request.contains("authorization: bearer channel-token"));
    }

    // UPD-028 — a server answering 401 or 403 to a channel that sends headers refuses
    // this build's access.
    #[tokio::test]
    async fn a_401_or_403_answer_to_a_channel_with_headers_is_a_refusal() {
        for status in ["401 Unauthorized", "403 Forbidden"] {
            let (endpoint, served) = serve_once(status).await;

            let refused =
                access_refused(&channel(endpoint, &[("authorization", "Bearer expired")])).await;

            assert!(refused, "{status}");
            assert!(served
                .await
                .expect("served")
                .contains("authorization: bearer expired"));
        }
    }

    // UPD-028 — a download can be refused where the check was not: the installer's own
    // address is asked, not the endpoint.
    #[tokio::test]
    async fn a_refused_installer_address_is_a_refusal_even_when_the_endpoint_answers() {
        let (endpoint, _endpoint_served) = serve_once("200 OK").await;
        let (installer, installer_served) = serve_once("403 Forbidden").await;
        let channel = channel(endpoint, &[("authorization", "Bearer expired")]);

        assert!(access_refused_at(&installer, &channel).await);
        assert!(installer_served
            .await
            .expect("served")
            .contains("authorization: bearer expired"));
    }

    // UPD-028 — any other answer is not a refusal: a missing release stays a silent failure.
    #[tokio::test]
    async fn a_404_answer_is_not_a_refusal() {
        let (endpoint, _served) = serve_once("404 Not Found").await;

        assert!(!access_refused(&channel(endpoint, &[("authorization", "Bearer valid")])).await);
    }

    // UPD-021 — a channel without headers is never probed: nothing is sent, and the
    // anonymous failure stays silent.
    #[tokio::test]
    async fn a_channel_without_headers_is_never_probed() {
        let (endpoint, served) = serve_once("401 Unauthorized").await;

        assert!(!access_refused(&channel(endpoint, &[])).await);
        assert!(!served.is_finished());
        served.abort();
    }
}

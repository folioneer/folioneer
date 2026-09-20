//! Typed error for the update-checker use case (error-model gold).

/// Failures the update-checker commands can surface to the renderer.
///
/// `#[serde(tag = "code")]` serialises each variant as `{ "code": "..." }`,
/// matching the gold error model. Underlying infrastructure details (updater
/// plugin internals, OS paths, HTTP status codes) are logged server-side via
/// `tracing::error!` and never cross the IPC wire.
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(tag = "code")]
pub enum UpdateError {
    /// `install` was called before a successful `download` — no bytes are staged.
    #[error("No downloaded update available")]
    NoDownloadedUpdate,
    /// The update operation failed (updater init, fetch, or install). The
    /// underlying cause is logged server-side, not exposed on the wire.
    #[error("Update operation failed")]
    OperationFailed,
    /// The update server refuses this build's access — the credentials its update
    /// channel sends are missing, expired or revoked (UPD-028).
    #[error("Update access refused")]
    AccessRefused,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_downloaded_update_serialises_with_code_tag() {
        let value = serde_json::to_value(UpdateError::NoDownloadedUpdate).unwrap();
        assert_eq!(value, json!({ "code": "NoDownloadedUpdate" }));
    }

    #[test]
    fn access_refused_serialises_with_code_tag() {
        let value = serde_json::to_value(UpdateError::AccessRefused).unwrap();
        assert_eq!(value, json!({ "code": "AccessRefused" }));
    }

    #[test]
    fn operation_failed_serialises_with_code_tag() {
        let value = serde_json::to_value(UpdateError::OperationFailed).unwrap();
        assert_eq!(value, json!({ "code": "OperationFailed" }));
    }
}

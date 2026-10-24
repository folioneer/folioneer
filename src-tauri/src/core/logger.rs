//! Logging infrastructure and frontend log bridge.

/// Context identifier for frontend-initiated log messages.
pub const FRONTEND: &str = "frontend";

/// Context identifier for backend-originated log messages.
pub const BACKEND: &str = "backend";

/// The longest message from the window written to the log, in characters.
#[cfg(any(feature = "app", test))]
const LOG_MESSAGE_LENGTH: usize = 2_000;

/// A message from the window as one line of the log: its control characters and line
/// separators escaped, so it cannot write a line of its own, and cut at
/// `LOG_MESSAGE_LENGTH` with an ellipsis.
#[cfg(any(feature = "app", test))]
fn as_one_line(message: &str) -> String {
    let mut line = String::new();
    for character in message.chars().take(LOG_MESSAGE_LENGTH) {
        if character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') {
            line.extend(character.escape_default());
        } else {
            line.push(character);
        }
    }
    if message.chars().nth(LOG_MESSAGE_LENGTH).is_some() {
        line.push('…');
    }
    line
}

/// Tauri command allowing the frontend to emit structured log entries
/// into the backend tracing system (visible in app logs and collect-logs output).
#[cfg(feature = "app")]
#[tauri::command]
#[specta::specta]
pub fn log_frontend(level: String, message: String) {
    let message = as_one_line(&message);
    match level.as_str() {
        "trace" => tracing::trace!(target: FRONTEND, "{}", message),
        "debug" => tracing::debug!(target: FRONTEND, "{}", message),
        "info" => tracing::info!(target: FRONTEND, "{}", message),
        "warn" => tracing::warn!(target: FRONTEND, "{}", message),
        "error" => tracing::error!(target: FRONTEND, "{}", message),
        _ => tracing::info!(target: FRONTEND, "{}", message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A message from the window is one bounded line in the log: its line breaks and other
    // control characters are escaped, so it cannot forge a line of its own.
    #[test]
    fn a_window_message_is_one_bounded_line() {
        assert_eq!(
            as_one_line("saved\n2026-10-24 ERROR backend: forged\r\tend"),
            "saved\\n2026-10-24 ERROR backend: forged\\r\\tend"
        );
        let long = "é".repeat(LOG_MESSAGE_LENGTH + 10);
        assert_eq!(as_one_line(&long).chars().count(), LOG_MESSAGE_LENGTH + 1);
        assert!(as_one_line(&long).ends_with('…'));
        assert_eq!(as_one_line("a\u{2028}b"), "a\\u{2028}b");
        assert_eq!(as_one_line("plain"), "plain");
    }
}

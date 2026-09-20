use serde::Serialize;
use specta::Type;

/// Flat error enum for the scheduled-fetch use case (`configure_scheduled_fetch`,
/// `get_scheduled_fetch_status`). Use-case-owned persistence (spec Context
/// divergence) means there is no bounded-context enum to wrap — this is the
/// entire wire-facing failure surface (SPF-013, SPF-019).
#[derive(Debug, thiserror::Error, Serialize, Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum ScheduledFetchError {
    /// The trigger time is not a well-formed "HH:MM" time of day (SPF-019).
    #[error("Trigger time is not a valid time of day")]
    InvalidTriggerTime,
    /// The OS schedule could not be registered (SPF-013).
    #[error("Failed to register the daily schedule with the operating system")]
    ScheduleRegistrationFailed,
    /// The OS schedule could not be removed (SPF-013).
    #[error("Failed to remove the daily schedule from the operating system")]
    ScheduleRemovalFailed,
    /// An unexpected database error occurred.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — every variant emits a flat { "code": "..." }.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(ScheduledFetchError::InvalidTriggerTime).unwrap(),
            json!({ "code": "InvalidTriggerTime" })
        );
        assert_eq!(
            to_value(ScheduledFetchError::ScheduleRegistrationFailed).unwrap(),
            json!({ "code": "ScheduleRegistrationFailed" })
        );
        assert_eq!(
            to_value(ScheduledFetchError::ScheduleRemovalFailed).unwrap(),
            json!({ "code": "ScheduleRemovalFailed" })
        );
        assert_eq!(
            to_value(ScheduledFetchError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}

use crate::context::account::error::AccountError;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;

/// Direction of a holding-note price alarm relative to its threshold (HNO-011).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Type)]
pub enum ThresholdDirection {
    /// Triggers when the current price falls strictly below the threshold (HNO-030).
    Below,
    /// Triggers when the current price rises strictly above the threshold (HNO-030).
    Above,
}

impl std::fmt::Display for ThresholdDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ThresholdDirection::Below => "Below",
            ThresholdDirection::Above => "Above",
        })
    }
}

impl std::str::FromStr for ThresholdDirection {
    type Err = ();

    fn from_str(value: &str) -> StdResult<Self, Self::Err> {
        match value {
            "Below" => Ok(ThresholdDirection::Below),
            "Above" => Ok(ThresholdDirection::Above),
            _ => Err(()),
        }
    }
}

/// A free-text note pinned to an (account, asset) holding pair, with an optional
/// price alarm — at most one note per pair (HNO-010).
///
/// `threshold_price` is a nominal share price in asset-currency micros (HNO-031);
/// the alarm carries both `threshold_price` and `threshold_direction` or neither
/// (HNO-011).
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct HoldingNote {
    /// The owning account (PK part).
    pub account_id: String,
    /// The held asset (PK part).
    pub asset_id: String,
    /// Note text, trimmed, 1-500 chars (HNO-011).
    pub text: String,
    /// Alarm threshold as a nominal asset-currency share price in micros (HNO-031).
    pub threshold_price: Option<i64>,
    /// Alarm direction relative to the threshold (HNO-030).
    pub threshold_direction: Option<ThresholdDirection>,
}

impl HoldingNote {
    /// Creates a new HoldingNote, storing the trimmed text.
    ///
    /// HNO-011 — validates: trimmed text non-empty (`NoteTextEmpty`) and at most
    /// 500 characters (`NoteTextTooLong`); an alarm carries both fields or
    /// neither (`ThresholdIncomplete`) and a strictly positive threshold
    /// (`ThresholdNotPositive`).
    pub fn new(
        account_id: String,
        asset_id: String,
        text: String,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> StdResult<Self, AccountError> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(AccountError::NoteTextEmpty);
        }
        if text.chars().count() > 500 {
            return Err(AccountError::NoteTextTooLong);
        }
        match (threshold_price, threshold_direction) {
            (Some(price), Some(_)) if price <= 0 => return Err(AccountError::ThresholdNotPositive),
            (Some(_), Some(_)) | (None, None) => {}
            _ => return Err(AccountError::ThresholdIncomplete),
        }
        Ok(Self {
            account_id,
            asset_id,
            text,
            threshold_price,
            threshold_direction,
        })
    }

    /// Reconstructs a HoldingNote from storage without validation.
    pub fn from_storage(
        account_id: String,
        asset_id: String,
        text: String,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> Self {
        Self {
            account_id,
            asset_id,
            text,
            threshold_price,
            threshold_direction,
        }
    }

    /// HNO-030 — stateless live trigger: `Below` triggers when
    /// `current_price < threshold_price`, `Above` when
    /// `current_price > threshold_price` — strict comparisons; equality with the
    /// threshold triggers neither. No alarm or no price → not triggered.
    pub fn alarm_triggered(&self, current_price: Option<i64>) -> bool {
        match (
            self.threshold_price,
            self.threshold_direction,
            current_price,
        ) {
            (Some(threshold), Some(direction), Some(price)) => match direction {
                ThresholdDirection::Below => price < threshold,
                ThresholdDirection::Above => price > threshold,
            },
            _ => false,
        }
    }
}

/// Interface for holding note persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait HoldingNoteRepository: Send + Sync {
    /// Inserts or fully replaces the note for the note's (account, asset) pair (HNO-020).
    async fn upsert(&self, note: &HoldingNote) -> Result<()>;
    /// Deletes the note for the (account, asset) pair. No-op if absent (HNO-021).
    async fn delete(&self, account_id: &str, asset_id: &str) -> Result<()>;
    /// Fetches all notes of one account (HNO-040).
    async fn get_for_account(&self, account_id: &str) -> Result<Vec<HoldingNote>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_note(
        text: &str,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> StdResult<HoldingNote, AccountError> {
        HoldingNote::new(
            "acc-1".to_string(),
            "asset-1".to_string(),
            text.to_string(),
            threshold_price,
            threshold_direction,
        )
    }

    // HNO-011 — text must be non-empty after trimming
    #[test]
    fn empty_text_is_rejected() {
        assert!(matches!(
            make_note("", None, None),
            Err(AccountError::NoteTextEmpty)
        ));
        assert!(matches!(
            make_note("   \t\n", None, None),
            Err(AccountError::NoteTextEmpty)
        ));
    }

    // HNO-011 — text is stored trimmed
    #[test]
    fn text_is_stored_trimmed() {
        let note = make_note("  buy 7 shares below 150  ", None, None).unwrap();
        assert_eq!(note.text, "buy 7 shares below 150");
    }

    // HNO-011 — text over 500 chars (after trimming) is rejected; exactly 500 passes
    #[test]
    fn text_over_500_chars_is_rejected() {
        let long = "x".repeat(501);
        assert!(matches!(
            make_note(&long, None, None),
            Err(AccountError::NoteTextTooLong)
        ));
        let boundary = "é".repeat(500); // chars, not bytes
        assert!(make_note(&boundary, None, None).is_ok());
    }

    // HNO-011 — an alarm threshold must be strictly positive
    #[test]
    fn non_positive_threshold_is_rejected() {
        assert!(matches!(
            make_note("n", Some(0), Some(ThresholdDirection::Below)),
            Err(AccountError::ThresholdNotPositive)
        ));
        assert!(matches!(
            make_note("n", Some(-150_000_000), Some(ThresholdDirection::Above)),
            Err(AccountError::ThresholdNotPositive)
        ));
    }

    // HNO-011 — both alarm fields or neither
    #[test]
    fn half_specified_alarm_is_rejected() {
        assert!(matches!(
            make_note("n", Some(150_000_000), None),
            Err(AccountError::ThresholdIncomplete)
        ));
        assert!(matches!(
            make_note("n", None, Some(ThresholdDirection::Below)),
            Err(AccountError::ThresholdIncomplete)
        ));
    }

    // HNO-011 — a complete alarm and a plain no-alarm note are both valid
    #[test]
    fn complete_alarm_and_no_alarm_are_valid() {
        let with_alarm = make_note("n", Some(150_000_000), Some(ThresholdDirection::Below));
        assert!(with_alarm.is_ok());
        let without_alarm = make_note("n", None, None);
        assert!(without_alarm.is_ok());
    }

    // HNO-030 — strict comparisons; equality triggers neither; None price → false
    #[test]
    fn alarm_triggered_truth_table() {
        let below = make_note("n", Some(150_000_000), Some(ThresholdDirection::Below)).unwrap();
        assert!(below.alarm_triggered(Some(149_999_999)));
        assert!(!below.alarm_triggered(Some(150_000_000))); // equality → neither
        assert!(!below.alarm_triggered(Some(150_000_001)));
        assert!(!below.alarm_triggered(None));

        let above = make_note("n", Some(150_000_000), Some(ThresholdDirection::Above)).unwrap();
        assert!(above.alarm_triggered(Some(150_000_001)));
        assert!(!above.alarm_triggered(Some(150_000_000))); // equality → neither
        assert!(!above.alarm_triggered(Some(149_999_999)));
        assert!(!above.alarm_triggered(None));

        let no_alarm = make_note("n", None, None).unwrap();
        assert!(!no_alarm.alarm_triggered(Some(1)));
        assert!(!no_alarm.alarm_triggered(None));
    }
}

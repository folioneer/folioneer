//! The shape a received change must have before the engine sees it (SYN-034): its identity
//! is the canonical identity its own content derives (CFR-012) — so the record the engine
//! decides about is the record the owning context writes — its content is present exactly
//! when it is not a removal, and its amounts and texts stay within the bounds this
//! application's own records stay within. A change that fails makes the whole segment it
//! came in unreadable: nothing from it is applied.

use serde_json::Value;

use crate::shared::domain::{Operation, RecordIdentity, RecordKind};

/// The widest amount, quantity, price, or rate a record may carry, in micros (±10⁹ units).
pub const AMOUNT_BOUND: i64 = 1_000_000_000_000_000;
/// The longest text a record may carry, in characters (the holding note's own cap, HNO-011).
pub const TEXT_BOUND: usize = 500;

/// Why a received change is not taken in (SYN-034).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MalformedChange {
    /// Its logical timestamp or `based_on` is not a counter (CFR-010).
    #[error("logical timestamp is not a counter")]
    NotACounter,
    /// A creation or update carries no content, or content that is not a JSON object.
    #[error("content missing or not an object")]
    ContentMissing,
    /// Its identity is not the canonical identity of its kind (CFR-012), or — for a creation
    /// or update — not the one its content derives.
    #[error("identity {declared:?} does not match the content's {derived:?}")]
    IdentityMismatch {
        /// The identity the change declares.
        declared: String,
        /// The identity its content derives; the expected shape when content is absent.
        derived: String,
    },
    /// A number is not an integer within `±AMOUNT_BOUND`.
    #[error("field {field} is out of bounds")]
    AmountOutOfBounds {
        /// The offending field, dotted when nested.
        field: String,
    },
    /// A text is longer than `TEXT_BOUND` characters.
    #[error("field {field} is too long")]
    TextTooLong {
        /// The offending field, dotted when nested.
        field: String,
    },
}

/// The content fields a record's canonical identity is built from (CFR-012), per kind —
/// the same key the recording side hands `RecordIdentity::canonical`.
fn identity_fields(kind: RecordKind) -> &'static [&'static str] {
    match kind {
        RecordKind::Account
        | RecordKind::Category
        | RecordKind::Asset
        | RecordKind::Transaction => &["id"],
        RecordKind::FeeSchedule | RecordKind::FeeCatchUpPosition | RecordKind::HoldingNote => {
            &["account_id", "asset_id"]
        }
        RecordKind::AssetPrice => &["asset_id", "date"],
        RecordKind::CurrencyPair => &["from_currency", "to_currency"],
        RecordKind::CurrencyRate => &["from_currency", "to_currency", "date"],
    }
}

/// CFR-012 — whether `identity` has the shape of `kind`'s canonical identity: one non-empty
/// part per key field.
fn identity_well_formed(kind: RecordKind, identity: &str) -> bool {
    let parts: Vec<&str> = identity.split(':').collect();
    parts.len() == identity_fields(kind).len() && parts.iter().all(|part| !part.is_empty())
}

/// The canonical identity `content` derives for `kind`; `None` when a key field is missing.
fn derived_identity(kind: RecordKind, content: &Value) -> Option<RecordIdentity> {
    let parts: Option<Vec<&str>> = identity_fields(kind)
        .iter()
        .map(|field| content.get(field)?.as_str())
        .collect();
    parts.map(|parts| RecordIdentity::canonical(kind, &parts))
}

/// Every number within `±AMOUNT_BOUND` and every text within `TEXT_BOUND`, at any depth.
fn bounded(value: &Value, path: &str) -> Result<(), MalformedChange> {
    match value {
        Value::Number(number) => match number.as_i64() {
            Some(amount) if amount.abs() <= AMOUNT_BOUND => Ok(()),
            _ => Err(MalformedChange::AmountOutOfBounds {
                field: path.to_string(),
            }),
        },
        Value::String(text) if text.chars().count() > TEXT_BOUND => {
            Err(MalformedChange::TextTooLong {
                field: path.to_string(),
            })
        }
        Value::Object(fields) => fields.iter().try_for_each(|(name, value)| {
            let path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            bounded(value, &path)
        }),
        Value::Array(items) => items.iter().try_for_each(|item| bounded(item, path)),
        _ => Ok(()),
    }
}

/// SYN-034/CFR-012 — checks one received change of `kind`, declared to be about
/// `identity`, with `content` (absent for a removal): its identity is well-formed and, for a
/// creation or update, equals the one its content derives; and its content is bounded.
pub fn check_received_change(
    kind: RecordKind,
    identity: &str,
    operation: Operation,
    content: Option<&str>,
) -> Result<(), MalformedChange> {
    if !identity_well_formed(kind, identity) {
        return Err(MalformedChange::IdentityMismatch {
            declared: identity.to_string(),
            derived: identity_fields(kind).join(":"),
        });
    }
    if operation == Operation::Removed {
        return Ok(());
    }
    let value: Value = content
        .and_then(|content| serde_json::from_str(content).ok())
        .filter(Value::is_object)
        .ok_or(MalformedChange::ContentMissing)?;
    let derived = derived_identity(kind, &value).ok_or(MalformedChange::ContentMissing)?;
    if derived.as_str() != identity {
        return Err(MalformedChange::IdentityMismatch {
            declared: identity.to_string(),
            derived: derived.as_str().to_string(),
        });
    }
    bounded(&value, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    // CFR-012 — a change whose declared identity is not the one its content derives is
    // malformed: the engine would decide about record A while the context writes record B.
    #[test]
    fn identity_must_match_the_contents_own_key() {
        let mismatch = check_received_change(
            RecordKind::Transaction,
            "tx-a",
            Operation::Created,
            Some(r#"{"id":"tx-b","account_id":"account-1"}"#),
        );
        assert_eq!(
            mismatch,
            Err(MalformedChange::IdentityMismatch {
                declared: "tx-a".into(),
                derived: "tx-b".into(),
            })
        );
        assert_eq!(
            check_received_change(
                RecordKind::Transaction,
                "tx-a",
                Operation::Created,
                Some(r#"{"id":"tx-a","account_id":"account-1"}"#),
            ),
            Ok(())
        );
    }

    // CFR-012 — composite identities derive from their key fields in canonical order.
    #[test]
    fn composite_identity_derives_from_its_key_fields() {
        assert_eq!(
            check_received_change(
                RecordKind::CurrencyRate,
                "USD:EUR:2026-08-20",
                Operation::Created,
                Some(
                    r#"{"from_currency":"USD","to_currency":"EUR","date":"2026-08-20","rate":920000}"#
                ),
            ),
            Ok(())
        );
        assert!(matches!(
            check_received_change(
                RecordKind::HoldingNote,
                "account-1:asset-2",
                Operation::Updated,
                Some(r#"{"account_id":"account-1","asset_id":"asset-9","text":"n"}"#),
            ),
            Err(MalformedChange::IdentityMismatch { .. })
        ));
    }

    // A removal carries no content: only the identity's shape is checked.
    #[test]
    fn removal_checks_identity_shape_only() {
        assert_eq!(
            check_received_change(
                RecordKind::FeeSchedule,
                "account-1:asset-1",
                Operation::Removed,
                None
            ),
            Ok(())
        );
        assert!(matches!(
            check_received_change(
                RecordKind::FeeSchedule,
                "account-1",
                Operation::Removed,
                None
            ),
            Err(MalformedChange::IdentityMismatch { .. })
        ));
        assert!(matches!(
            check_received_change(RecordKind::Account, "", Operation::Removed, None),
            Err(MalformedChange::IdentityMismatch { .. })
        ));
    }

    // A creation or update must carry an object as content.
    #[test]
    fn creation_without_content_is_malformed() {
        assert_eq!(
            check_received_change(RecordKind::Account, "account-1", Operation::Created, None),
            Err(MalformedChange::ContentMissing)
        );
        assert_eq!(
            check_received_change(
                RecordKind::Account,
                "account-1",
                Operation::Created,
                Some("[]")
            ),
            Err(MalformedChange::ContentMissing)
        );
    }

    // An amount beyond ±10¹⁵ micros is malformed, at any depth; the bound itself is fine.
    #[test]
    fn amount_beyond_the_bound_is_malformed() {
        let content = format!(r#"{{"id":"tx-1","quantity":{}}}"#, AMOUNT_BOUND + 1);
        assert_eq!(
            check_received_change(
                RecordKind::Transaction,
                "tx-1",
                Operation::Created,
                Some(&content)
            ),
            Err(MalformedChange::AmountOutOfBounds {
                field: "quantity".into()
            })
        );
        let content = format!(r#"{{"id":"tx-1","quantity":{}}}"#, -AMOUNT_BOUND);
        assert_eq!(
            check_received_change(
                RecordKind::Transaction,
                "tx-1",
                Operation::Created,
                Some(&content)
            ),
            Ok(())
        );
        let nested = r#"{"id":"asset-1","category":{"id":"cat-1","weight":1e30}}"#;
        assert_eq!(
            check_received_change(
                RecordKind::Asset,
                "asset-1",
                Operation::Created,
                Some(nested)
            ),
            Err(MalformedChange::AmountOutOfBounds {
                field: "category.weight".into()
            })
        );
    }

    // A text longer than 500 characters is malformed; 500 is fine.
    #[test]
    fn text_beyond_the_bound_is_malformed() {
        let long = "x".repeat(TEXT_BOUND + 1);
        let content = format!(r#"{{"id":"account-1","name":"{long}"}}"#);
        assert_eq!(
            check_received_change(
                RecordKind::Account,
                "account-1",
                Operation::Updated,
                Some(&content)
            ),
            Err(MalformedChange::TextTooLong {
                field: "name".into()
            })
        );
        let exact = "é".repeat(TEXT_BOUND);
        let content = format!(r#"{{"id":"account-1","name":"{exact}"}}"#);
        assert_eq!(
            check_received_change(
                RecordKind::Account,
                "account-1",
                Operation::Updated,
                Some(&content)
            ),
            Ok(())
        );
    }
}

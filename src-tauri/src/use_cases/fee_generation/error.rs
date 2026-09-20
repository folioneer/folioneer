use crate::context::account::AccountError;

/// Use-case composite for the **apply_due_fee_deductions** failure surface (FEE-040+).
///
/// Wraps account-BC failures that may surface during batch fee deduction
/// (e.g. `DatabaseError` when loading schedules or saving transactions).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum FeeGenerationError {
    /// Account-BC rejection (lookup, infra, transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
}

#[cfg(test)]
mod fee_generation_error_wire_tests {
    use super::*;

    /// error-model.md — every `FeeGenerationError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression).
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<FeeGenerationError> = vec![
            AccountError::DatabaseError.into(),
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize FeeGenerationError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "FeeGenerationError variant did not emit a string `code`: {value}"
            );
        }
    }
}

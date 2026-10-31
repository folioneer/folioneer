/// Refusals of `set_price_provider_choice` (MKT-214).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum PriceProviderChoiceError {
    /// This build does not offer that External provider.
    #[error("This build does not offer that price provider")]
    NotOffered,
    /// The choice could not be kept on this computer; nothing changed.
    #[error("The choice could not be saved on this computer")]
    NotSaved,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — every variant emits a flat { "code": "…" }.
    #[test]
    fn every_variant_is_a_flat_code() {
        assert_eq!(
            to_value(PriceProviderChoiceError::NotOffered).expect("serialized"),
            json!({ "code": "NotOffered" })
        );
        assert_eq!(
            to_value(PriceProviderChoiceError::NotSaved).expect("serialized"),
            json!({ "code": "NotSaved" })
        );
    }
}

//! AGT-014, CLI-021 — what an agent and the command line's JSON read: every figure as exact
//! decimal text, as they write it, so neither of them sees a scale.

use serde_json::Value;

use super::named_recording::decimal;

/// The whole numbers that are not figures: what counts things, and a split's ratio.
pub const WHOLE_NUMBERS: [&str; 7] = [
    "new",
    "old",
    "outdated_price_count",
    "page",
    "pages",
    "total",
    "total_holding_count",
];

/// `value` with every figure as decimal text, at any depth, and no field named after a scale.
/// A number under an unknown name is taken for a figure, so a new field cannot leave as a
/// scaled integer.
pub fn figures_as_text(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(figures_as_text).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(name, field)| {
                    let shown = match field {
                        Value::Number(number) if !WHOLE_NUMBERS.contains(&name.as_str()) => {
                            Value::String(
                                // Every figure is an `i64`, so the fallback is never a figure's.
                                number.as_i64().map_or_else(|| number.to_string(), decimal),
                            )
                        }
                        other => figures_as_text(other),
                    };
                    let name = name.strip_suffix("_micros").unwrap_or(&name).to_string();
                    (name, shown)
                })
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agt_014_every_figure_is_decimal_text_at_any_depth() {
        let read = figures_as_text(json!({
            "quantity": 12_500_000,
            "note": "kept",
            "archived": false,
            "average_price": null,
            "holdings": [{ "total_return_pct": -1_500_000, "market_value": 0 }],
        }));

        assert_eq!(
            read,
            json!({
                "quantity": "12.5",
                "note": "kept",
                "archived": false,
                "average_price": null,
                "holdings": [{ "total_return_pct": "-1.5", "market_value": "0" }],
            })
        );
    }

    #[test]
    fn agt_014_what_counts_things_stays_a_whole_number() {
        let read = figures_as_text(json!({
            "total": 3,
            "page": 1,
            "pages": 1,
            "outdated_price_count": 2,
            "total_holding_count": 4,
            "unrecorded_splits": [{ "new": 2, "old": 1 }],
        }));

        assert_eq!(read["total"], 3);
        assert_eq!(read["unrecorded_splits"][0]["new"], 2);
        assert_eq!(read["total_holding_count"], 4);
    }

    #[test]
    fn agt_014_a_new_field_cannot_leave_as_a_scaled_integer() {
        assert_eq!(
            figures_as_text(json!({ "a_field_not_yet_written": 1_500_000 })),
            json!({ "a_field_not_yet_written": "1.5" })
        );
    }

    #[test]
    fn agt_014_no_field_is_named_after_a_scale() {
        assert_eq!(
            figures_as_text(json!({ "fee_rate_percent_micros": 500_000 })),
            json!({ "fee_rate_percent": "0.5" })
        );
    }
}

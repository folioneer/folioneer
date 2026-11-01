//! The scale of a quantity of a non-cash asset (TRX-024): hundred-millionths of a unit.
//! Money, prices, rates, a split's factor and the Cash Holding's balance stay in millionths.
//!
//! The window, the agent connection and the command line exchange quantities as decimal text
//! (TRX-024B). A CSV import still reads a file's figures in millionths: `from_surface`.

use crate::core::cash::is_cash_asset;

/// One unit of a non-cash asset, as a stored quantity.
pub const QUANTITY_UNIT: i128 = 100_000_000;

/// The largest quantity one transaction carries, here and on every other device (TRX-024C,
/// SYN-034): a billion units of a non-cash asset.
pub const LARGEST_QUANTITY: i64 = 100_000_000_000_000_000;

/// The decimals of a quantity of a non-cash asset (TRX-024).
const QUANTITY_DECIMALS: u32 = 8;

/// The decimals of money, and so of the Cash Holding's balance, a split's factor and a
/// dividend's amount (TRX-024).
pub const MONEY_DECIMALS: u32 = 6;

/// Why a decimal text is not a figure (TRX-024B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigureTextError {
    /// Not digits with one dot and an optional leading minus, or more decimals than its scale.
    NotAFigure,
    /// More than storage holds.
    TooLarge,
}

/// `value`, an integer of `decimals` decimals, as decimal text without trailing zeros.
pub fn scaled_text(value: i64, decimals: u32) -> String {
    let unit = 10_u64.pow(decimals);
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.unsigned_abs();
    let (whole, fraction) = (absolute / unit, absolute % unit);
    if fraction == 0 {
        return format!("{sign}{whole}");
    }
    let width = decimals as usize;
    let fraction = format!("{fraction:0width$}");
    format!("{sign}{whole}.{}", fraction.trim_end_matches('0'))
}

/// A decimal text with a dot and at most `decimals` decimals, as an integer of that scale.
/// A leading minus is read, so the core refuses a negative figure with its own reason.
pub fn read_scaled(text: &str, decimals: u32) -> Result<i64, FigureTextError> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let is_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty())
        || !is_digits(whole)
        || !is_digits(fraction)
        || fraction.len() > decimals as usize
    {
        return Err(FigureTextError::NotAFigure);
    }
    let scaled = format!("{whole}{fraction:0<width$}", width = decimals as usize);
    let read: i128 = scaled.parse().map_err(|_| FigureTextError::TooLarge)?;
    let signed = if negative { -read } else { read };
    i64::try_from(signed).map_err(|_| FigureTextError::TooLarge)
}

/// TRX-024B — a stored quantity of a non-cash asset as decimal text.
pub fn non_cash_quantity_text(quantity: i64) -> String {
    scaled_text(quantity, QUANTITY_DECIMALS)
}

/// TRX-024B — how a read model gives a stored quantity of a non-cash asset to a surface.
pub fn as_text<S: serde::Serializer>(quantity: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&non_cash_quantity_text(*quantity))
}

/// As `as_text`, for a quantity that may be absent.
pub fn as_optional_text<S: serde::Serializer>(
    quantity: &Option<i64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match quantity {
        Some(quantity) => as_text(quantity, serializer),
        None => serializer.serialize_none(),
    }
}

/// TRX-024B — a stored quantity of `asset_id` as decimal text.
pub fn quantity_text(asset_id: &str, quantity: i64) -> String {
    scaled_text(quantity, decimals_of(asset_id))
}

/// TRX-024B — a quantity of a non-cash asset, sent as decimal text, as stored.
pub fn stored_non_cash_quantity(text: &str) -> Result<i64, FigureTextError> {
    read_scaled(text, QUANTITY_DECIMALS)
}

/// TRX-024B — a quantity of `asset_id`, sent as decimal text, as stored.
pub fn stored_quantity(asset_id: &str, text: &str) -> Result<i64, FigureTextError> {
    read_scaled(text, decimals_of(asset_id))
}

/// How many decimals a quantity of `asset_id` carries.
fn decimals_of(asset_id: &str) -> u32 {
    if is_cash_asset(asset_id) {
        MONEY_DECIMALS
    } else {
        QUANTITY_DECIMALS
    }
}

/// How many stored steps make one millionth of a unit.
const SURFACE_STEP: i64 = 100;

/// A quantity a surface sent in millionths, as stored. One past what storage holds becomes
/// `i64::MAX`, which recording refuses (`Transaction` validation, TRX-020).
pub fn from_surface(quantity: i64) -> i64 {
    quantity.saturating_mul(SURFACE_STEP)
}

/// TRX-024 — a test's figure in millionths for an asset, as stored: the cash line keeps its
/// own scale.
#[cfg(test)]
pub fn stored_for(asset_id: &str, quantity: i64) -> i64 {
    if is_cash_asset(asset_id) {
        quantity
    } else {
        from_surface(quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trx_024_the_cash_line_keeps_its_scale_at_the_surface() {
        let cash = crate::core::cash::system_cash_asset_id("EUR");

        assert_eq!(stored_for(&cash, 1_500_000), 1_500_000);
        assert_eq!(stored_for("asset", 1_500_000), 150_000_000);
    }

    #[test]
    fn trx_024b_a_satoshi_reads_back_as_it_was_typed() {
        assert_eq!(stored_quantity("BTC", "0.00000001"), Ok(1));
        assert_eq!(quantity_text("BTC", 1), "0.00000001");
        assert_eq!(quantity_text("BTC", 150_000_000), "1.5");
        assert_eq!(quantity_text("BTC", -150_000_000), "-1.5");
        assert_eq!(quantity_text("BTC", 0), "0");
    }

    #[test]
    fn trx_024b_a_cash_quantity_and_money_keep_six_decimals() {
        let cash = crate::core::cash::system_cash_asset_id("EUR");
        assert_eq!(stored_quantity(&cash, "1.5"), Ok(1_500_000));
        assert_eq!(quantity_text(&cash, 1_500_000), "1.5");
        assert_eq!(
            stored_quantity(&cash, "0.0000001"),
            Err(FigureTextError::NotAFigure)
        );
    }

    #[test]
    fn trx_024g_what_is_not_a_quantity_is_refused() {
        for text in [
            "",
            " ",
            ".",
            "-",
            "1,5",
            "1e3",
            "1.2.3",
            "abc",
            "0.000000001",
            "1.000000000",
            "+5",
        ] {
            assert_eq!(
                stored_quantity("BTC", text),
                Err(FigureTextError::NotAFigure),
                "{text:?}"
            );
        }
        assert_eq!(stored_quantity("BTC", "-2"), Ok(-200_000_000));
        assert_eq!(stored_quantity("BTC", ".5"), Ok(50_000_000));
        assert_eq!(stored_quantity("BTC", " 2 "), Ok(200_000_000));
    }

    #[test]
    fn trx_024g_a_quantity_past_storage_is_too_large() {
        assert_eq!(stored_quantity("BTC", "92233720368.54775807"), Ok(i64::MAX));
        for text in [
            "92233720368.54775808",
            "99999999999999999999999999999999999999999",
        ] {
            assert_eq!(
                stored_quantity("BTC", text),
                Err(FigureTextError::TooLarge),
                "{text}"
            );
        }
    }

    #[test]
    fn trx_024_a_figure_in_millionths_reads_on_the_stored_scale() {
        assert_eq!(from_surface(1_500_000), 150_000_000);
    }

    #[test]
    fn trx_024_a_surface_quantity_past_storage_saturates() {
        assert_eq!(from_surface(i64::MAX / 2), i64::MAX);
    }
}

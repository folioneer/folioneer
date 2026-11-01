//! The scale of a quantity of a non-cash asset (TRX-024): hundred-millionths of a unit.
//! Money, prices, rates, a split's factor and the Cash Holding's balance stay in millionths.
//!
//! The window, the agent connection and the command line still exchange quantities in
//! millionths (TRX-024); they cross that boundary through `to_surface` and `from_surface`.

use crate::core::cash::is_cash_asset;

/// One unit of a non-cash asset, as a stored quantity.
pub const QUANTITY_UNIT: i128 = 100_000_000;

/// The largest quantity one transaction carries, here and on every other device (TRX-024C,
/// SYN-034): a billion units of a non-cash asset.
pub const LARGEST_QUANTITY: i64 = 100_000_000_000_000_000;

/// How many stored steps make one millionth of a unit.
const SURFACE_STEP: i64 = 100;

/// A stored quantity as a surface gives it, in millionths, rounded toward zero.
pub fn to_surface(quantity: i64) -> i64 {
    quantity / SURFACE_STEP
}

/// A quantity a surface sent in millionths, as stored. One past what storage holds becomes
/// `i64::MAX`, which recording refuses (`Transaction` validation, TRX-020).
pub fn from_surface(quantity: i64) -> i64 {
    quantity.saturating_mul(SURFACE_STEP)
}

/// TRX-024 — a holding's quantity as a surface gives it: the cash line keeps its own scale.
pub fn shown_for(asset_id: &str, quantity: i64) -> i64 {
    if is_cash_asset(asset_id) {
        quantity
    } else {
        to_surface(quantity)
    }
}

/// TRX-024 — a quantity a surface sent for an asset, as stored: the cash line keeps its
/// own scale.
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
        assert_eq!(shown_for(&cash, 1_500_000), 1_500_000);
        assert_eq!(stored_for("asset", 1_500_000), 150_000_000);
        assert_eq!(shown_for("asset", 150_000_000), 1_500_000);
    }

    #[test]
    fn trx_024_a_surface_quantity_round_trips() {
        assert_eq!(from_surface(1_500_000), 150_000_000);
        assert_eq!(to_surface(from_surface(1_500_000)), 1_500_000);
    }

    #[test]
    fn trx_024_a_finer_stored_quantity_reaches_a_surface_rounded_toward_zero() {
        assert_eq!(to_surface(199), 1);
        assert_eq!(to_surface(-199), -1);
    }

    #[test]
    fn trx_024_a_surface_quantity_past_storage_saturates() {
        assert_eq!(from_surface(i64::MAX / 2), i64::MAX);
    }
}

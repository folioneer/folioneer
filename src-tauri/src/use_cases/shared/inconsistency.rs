//! CFR-042/SYN-040 — the derived holding inconsistency the account-reading use cases
//! (`account_details`, `account_summary`, `portfolio_sync`'s status) surface: a merged
//! ledger that oversold a position or overdrew the cash holding. Derived from the replayed
//! holding on every read, never stored, never synced (ADR-013).

use crate::context::account::Holding;
use crate::context::sync::HoldingInconsistency;
use crate::core::cash::is_cash_asset;
use crate::shared::domain::quantity::non_cash_quantity_text;

/// The inconsistency `holding` carries, if any: a negative quantity is `Oversold` for a
/// position and `CashOverdrawn` for the Cash Holding (CSH-080).
pub fn holding_inconsistency(holding: &Holding) -> Option<HoldingInconsistency> {
    if holding.quantity >= 0 {
        return None;
    }
    Some(if is_cash_asset(&holding.asset_id) {
        HoldingInconsistency::CashOverdrawn {
            amount: holding.quantity,
        }
    } else {
        HoldingInconsistency::Oversold {
            quantity: non_cash_quantity_text(holding.quantity),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(asset_id: &str, quantity: i64) -> Holding {
        Holding::restore(
            "h-1".into(),
            "account-1".into(),
            asset_id.into(),
            crate::shared::domain::quantity::stored_for(asset_id, quantity),
            1_000_000,
            0,
            None,
        )
    }

    // CFR-042 — a non-negative quantity is consistent; a negative position is Oversold; a
    // negative cash balance is CashOverdrawn.
    #[test]
    fn derives_the_inconsistency_from_the_sign_and_the_kind_of_holding() {
        assert_eq!(holding_inconsistency(&holding("asset-1", 0)), None);
        assert_eq!(
            holding_inconsistency(&holding("asset-1", -5_000_000)),
            Some(HoldingInconsistency::Oversold {
                quantity: "-5".to_string()
            })
        );
        assert_eq!(
            holding_inconsistency(&holding("system-cash-eur", -1)),
            Some(HoldingInconsistency::CashOverdrawn { amount: -1 })
        );
    }
}

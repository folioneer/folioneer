//! CBM — the cost of what a holding holds and of what leaves it, by the account's cost basis
//! method: one pool at its average cost, or lots taken the oldest first.

use super::country::CostBasisMethod;
use crate::shared::domain::quantity::QUANTITY_UNIT;

/// A split's factor is in millionths.
pub(crate) const MICRO: i128 = 1_000_000;

/// `value × numerator / denominator`, rounded down, for `0 ≤ numerator ≤ denominator`,
/// without the product overflowing: a lot's cost runs to 10²⁶ and its units to 10¹⁹.
fn share_of(value: i128, numerator: i128, denominator: i128) -> i128 {
    let (whole, rest) = (value / denominator, value % denominator);
    whole * numerator + rest * numerator / denominator
}

/// CBM-020 — under FIFO, the units one transaction brought in, with their acquisition date
/// and cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lot {
    /// The date of the transaction that brought the units in.
    pub(crate) acquired_on: String,
    /// The units still held, in quantity units.
    pub(crate) quantity: i128,
    /// What those units cost, in money micro-units × `QUANTITY_UNIT`, so a lot taken in part
    /// keeps the rest of its cost exactly.
    pub(crate) cost: i128,
}

/// What a removal took: the average price just before it, and the cost of what left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Taken {
    /// The holding's average price just before the removal (SEL-024, TRX-040).
    pub average_before: i64,
    /// The cost of the units taken, in money micro-units (CBM-022, SEL-024).
    pub cost: i64,
}

/// A holding's cost as its transactions are replayed: the quantity and its cost, and under
/// FIFO the lots that make them up.
#[derive(Debug, Clone)]
pub(crate) struct CostBasis {
    method: CostBasisMethod,
    quantity: i128,
    /// Σ cost, in money micro-units × `QUANTITY_UNIT` — the VWAP numerator (TRX-030).
    cost: i128,
    lots: Vec<Lot>,
}

impl CostBasis {
    pub(crate) fn new(method: CostBasisMethod) -> Self {
        Self {
            method,
            quantity: 0,
            cost: 0,
            lots: Vec::new(),
        }
    }

    fn fifo(&self) -> bool {
        self.method == CostBasisMethod::Fifo
    }

    pub(crate) fn quantity(&self) -> i128 {
        self.quantity
    }

    /// TRX-030 / CBM-027 — the cost divided by the quantity; none with nothing held.
    pub(crate) fn average_price(&self) -> Option<i64> {
        (self.quantity > 0).then(|| (self.cost / self.quantity) as i64)
    }

    /// CBM-020 / CBM-021 — units brought in on `acquired_on` at `total_amount` (zero for
    /// free shares and interest), after every lot acquired on or before that date.
    pub(crate) fn acquire(&mut self, acquired_on: &str, quantity: i64, total_amount: i64) {
        let cost = total_amount as i128 * QUANTITY_UNIT;
        self.quantity += quantity as i128;
        self.cost += cost;
        if self.fifo() {
            let at = self
                .lots
                .iter()
                .position(|lot| lot.acquired_on.as_str() > acquired_on)
                .unwrap_or(self.lots.len());
            self.lots.insert(
                at,
                Lot {
                    acquired_on: acquired_on.to_string(),
                    quantity: quantity as i128,
                    cost,
                },
            );
        }
    }

    /// CBM-022 / SEL-024 — `quantity` leaving: at the average price under the weighted
    /// average, from the oldest lots under FIFO.
    pub(crate) fn take(&mut self, quantity: i64) -> Taken {
        let average_before = self.average_price().unwrap_or(0);
        let wanted = quantity as i128;
        let taken = if self.fifo() {
            self.take_oldest(wanted)
        } else {
            average_before as i128 * wanted
        };
        self.quantity -= wanted;
        self.cost -= taken;
        Taken {
            average_before,
            cost: (taken / QUANTITY_UNIT) as i64,
        }
    }

    /// CBM-022 / CBM-028 — takes `wanted` units from the oldest lots and returns their cost; a
    /// lot with no units left gives up its cost to the first units taken after it.
    fn take_oldest(&mut self, mut wanted: i128) -> i128 {
        let mut taken = 0;
        while let Some(lot) = self.lots.first_mut() {
            if lot.quantity <= wanted {
                wanted -= lot.quantity;
                taken += lot.cost;
                self.lots.remove(0);
                if wanted == 0 && self.lots.first().is_none_or(|next| next.quantity > 0) {
                    break;
                }
            } else {
                let part = share_of(lot.cost, wanted, lot.quantity);
                lot.cost -= part;
                lot.quantity -= wanted;
                taken += part;
                break;
            }
        }
        taken
    }

    /// CBM-023 / FEE-023 — a fee in units: the quantity falls, the cost stays; under FIFO
    /// every lot loses the same share of its units.
    pub(crate) fn shrink(&mut self, quantity: i64) {
        let left = self.quantity - quantity as i128;
        if self.fifo() && self.quantity > 0 {
            let before = self.quantity;
            self.rescale_lots(|units| share_of(units, left.max(0), before), left.max(0));
        }
        self.quantity = left;
    }

    /// CBM-025 / SPL-020 — a split: the quantity rescales by `factor` (in millionths), the
    /// cost stays; under FIFO every lot rescales.
    pub(crate) fn split(&mut self, factor: i64) {
        let rescaled = self.quantity * factor as i128 / MICRO;
        if self.fifo() {
            self.rescale_lots(|units| units * factor as i128 / MICRO, rescaled.max(0));
        }
        self.quantity = rescaled;
    }

    /// Rescales every lot's units by `rescale`, the units rounding leaves over going to the
    /// most recent lot so the lots add up to `total` (CBM-023, CBM-025, CBM-026).
    fn rescale_lots(&mut self, rescale: impl Fn(i128) -> i128, total: i128) {
        let mut sum = 0;
        for lot in &mut self.lots {
            lot.quantity = rescale(lot.quantity);
            sum += lot.quantity;
        }
        if let Some(last) = self.lots.last_mut() {
            last.quantity += total - sum;
        }
    }

    /// The lots held, oldest first; empty under the weighted average.
    #[cfg(test)]
    pub(crate) fn lots(&self) -> &[Lot] {
        &self.lots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::quantity::from_surface;
    use CostBasisMethod::*;

    fn units(n: i64) -> i64 {
        from_surface(n * 1_000_000)
    }

    fn money(n: i64) -> i64 {
        n * 1_000_000
    }

    fn two_lots(method: CostBasisMethod) -> CostBasis {
        let mut basis = CostBasis::new(method);
        basis.acquire("2024-01-01", units(10), money(1_000));
        basis.acquire("2024-02-01", units(10), money(2_000));
        basis
    }

    // CBM-022 — a sale across two lots takes the oldest whole and the next in part; the
    // weighted average takes every unit at the average.
    #[test]
    fn a_sale_takes_the_oldest_lots_first() {
        let mut fifo = two_lots(Fifo);
        assert_eq!(fifo.take(units(15)).cost, money(2_000));
        assert_eq!(fifo.lots().len(), 1);
        assert_eq!(fifo.lots()[0].quantity, units(5) as i128);
        assert_eq!(fifo.average_price(), Some(money(200)));
        let mut average = two_lots(WeightedAverage);
        let taken = average.take(units(15));
        assert_eq!(
            (taken.average_before, taken.cost),
            (money(150), money(2_250))
        );
    }

    // CBM-021 — a lot acquired earlier than the last one is taken before it.
    #[test]
    fn lots_are_ordered_by_acquisition_date() {
        let mut basis = CostBasis::new(Fifo);
        basis.acquire("2024-03-01", units(1), money(300));
        basis.acquire("2024-01-01", units(1), money(100));
        basis.acquire("2024-03-01", units(1), money(301));
        let dates: Vec<&str> = basis
            .lots()
            .iter()
            .map(|l| l.acquired_on.as_str())
            .collect();
        assert_eq!(dates, ["2024-01-01", "2024-03-01", "2024-03-01"]);
        assert_eq!(basis.take(units(2)).cost, money(400));
    }

    // CBM-020 — free shares are a lot at no cost, taken in their turn.
    #[test]
    fn free_shares_are_a_lot_at_no_cost() {
        let mut basis = CostBasis::new(Fifo);
        basis.acquire("2024-01-01", units(10), money(1_000));
        basis.acquire("2024-02-01", units(10), 0);
        assert_eq!(basis.take(units(15)).cost, money(1_000));
        assert_eq!(basis.average_price(), Some(0));
    }

    // CBM-022 — a lot taken in part keeps the rest of its cost, so taking it all in thirds
    // costs exactly what it cost.
    #[test]
    fn a_lot_taken_in_parts_costs_exactly_its_cost() {
        let mut basis = CostBasis::new(Fifo);
        basis.acquire("2024-01-01", units(3), money(100));
        let costs: i64 = (0..3).map(|_| basis.take(units(1)).cost).sum();
        assert!((money(100) - costs).abs() <= 2, "{costs}");
        assert_eq!(basis.lots().len(), 0);
    }

    // CBM-023 — a fee shrinks every lot by the same share, the cost kept; the units rounding
    // leaves go to the most recent lot.
    #[test]
    fn a_fee_shrinks_every_lot() {
        let mut basis = two_lots(Fifo);
        basis.shrink(units(2));
        let left: Vec<i128> = basis.lots().iter().map(|l| l.quantity).collect();
        assert_eq!(left, [units(9) as i128, units(9) as i128]);
        assert_eq!(basis.quantity(), units(18) as i128);
        assert_eq!(basis.take(units(9)).cost, money(1_000));
        let mut odd = CostBasis::new(Fifo);
        odd.acquire("2024-01-01", 3, 0);
        odd.acquire("2024-01-02", 3, 0);
        odd.shrink(1);
        assert_eq!(odd.lots().iter().map(|l| l.quantity).sum::<i128>(), 5);
    }

    // CBM-025 — a split rescales every lot, the cost kept.
    #[test]
    fn a_split_rescales_every_lot() {
        let mut basis = two_lots(Fifo);
        basis.split(2_000_000);
        assert_eq!(basis.quantity(), units(40) as i128);
        assert_eq!(basis.take(units(25)).cost, money(1_500));
    }

    // CBM-028 — a cost left with no units leaves with the first units taken next under FIFO.
    #[test]
    fn a_cost_with_no_units_leaves_with_the_next_units() {
        let mut basis = CostBasis::new(Fifo);
        basis.acquire("2024-01-01", units(1), money(100));
        basis.shrink(units(1));
        basis.acquire("2024-02-01", units(1), money(50));
        assert_eq!(basis.average_price(), Some(money(150)));
        assert_eq!(basis.take(units(1)).cost, money(150));
    }

    // CBM-022 — a lot at the ceiling of what is recorded is taken in part without
    // overflowing: a cost of a trillion units of money for 10¹⁰ units.
    #[test]
    fn a_lot_at_the_ceiling_is_taken_in_part() {
        let mut basis = CostBasis::new(Fifo);
        let quantity = 10_000_000_000 * QUANTITY_UNIT as i64;
        basis.acquire("2024-01-01", quantity, 1_000_000_000_000_000_000);
        let taken = basis.take(quantity / 3);
        assert_eq!(taken.cost, 333_333_333_333_333_333);
        assert_eq!(
            basis.take(quantity - quantity / 3).cost,
            666_666_666_666_666_667
        );
    }

    // CBM-026 — the quantity is the same under either method.
    #[test]
    fn the_quantity_does_not_depend_on_the_method() {
        let run = |method| {
            let mut basis = two_lots(method);
            basis.take(units(3));
            basis.shrink(units(1));
            basis.split(1_500_000);
            basis.acquire("2024-01-15", units(2), 0);
            basis.quantity()
        };
        assert_eq!(run(Fifo), run(WeightedAverage));
    }
}

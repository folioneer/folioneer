//! The largest figure of money a record carries (TRX-020, SYN-034).

/// A trillion units of money in micro-units: the largest price, rate, fee or total a device
/// records, and so the largest every device reads from another (SYN-034) — room for a real
/// price in any currency (owner, 2026-11-07).
pub const LARGEST_AMOUNT: i64 = 1_000_000_000_000_000_000;

/// An i128 figure as an i64, held at the nearest end instead of wrapping: a figure past
/// either end is then refused by the limit (TRX-020).
pub fn saturate(figure: i128) -> i64 {
    figure.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

//! The largest figure of money a record carries (TRX-020, SYN-034).

/// A trillion units of money in micro-units: the largest price, rate, fee or total a device
/// records, and so the largest every device reads from another (SYN-034) — room for a real
/// price in any currency (owner, 2026-11-07).
pub const LARGEST_AMOUNT: i64 = 1_000_000_000_000_000_000;

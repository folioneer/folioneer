/// Cross-rate computation from a EUR-base snapshot (FXR-080/082/083).
///
/// Both external rate tiers publish EUR-base rates only (ADR-009). This function
/// computes `rate(from → to) = rate(EUR → to) / rate(EUR → from)` using i128
/// intermediates and integer division (truncation toward zero, per FXR-082).
///
/// The EUR leg itself is passed as `Some(1_000_000)` by the caller (1.0 in micros).
///
/// Returns `None` when either leg is `None` (FXR-083: missing leg makes the pair
/// unfetchable and must be skipped by the caller).
pub fn cross_rate_micros(eur_to_from: Option<i64>, eur_to_to: Option<i64>) -> Option<i64> {
    let eur_to_from = eur_to_from?;
    let eur_to_to = eur_to_to?;
    if eur_to_from == 0 {
        return None;
    }
    // i128 multiplication cannot overflow for i64 operands, but the cast back to
    // i64 is truncating (wrapping) — use a checked conversion so an out-of-range
    // result (only reachable on adversarial input) becomes an unfetchable pair
    // (None) rather than a silently wrong value.
    let result = eur_to_to as i128 * 1_000_000 / eur_to_from as i128;
    i64::try_from(result).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // FXR-080/082 — USD→EUR: from=USD (eur_to_from=1_164_600), to=EUR (eur_to_to=1_000_000)
    // rate = 1_000_000 * 1_000_000 / 1_164_600 = 858_663.91… → 858_663 (truncated toward zero)
    #[test]
    fn cross_rate_usd_to_eur() {
        let result = cross_rate_micros(Some(1_164_600), Some(1_000_000));
        assert_eq!(result, Some(858_663));
    }

    // FXR-080/082 — EUR→USD: from=EUR (eur_to_from=1_000_000), to=USD (eur_to_to=1_164_600)
    // rate = 1_164_600 * 1_000_000 / 1_000_000 = 1_164_600
    #[test]
    fn cross_rate_eur_to_usd() {
        let result = cross_rate_micros(Some(1_000_000), Some(1_164_600));
        assert_eq!(result, Some(1_164_600));
    }

    // FXR-080/082 — USD→GBP cross: eur_to_from=USD(1_164_600), eur_to_to=GBP(864_930)
    // rate = 864_930 * 1_000_000 / 1_164_600 = 742_684.18… → 742_684 (truncated toward zero)
    #[test]
    fn cross_rate_usd_to_gbp() {
        let result = cross_rate_micros(Some(1_164_600), Some(864_930));
        assert_eq!(result, Some(742_684));
    }

    // FXR-083 — missing `from` leg → None
    #[test]
    fn cross_rate_missing_from_leg_returns_none() {
        let result = cross_rate_micros(None, Some(1_000_000));
        assert_eq!(result, None);
    }

    // FXR-083 — missing `to` leg → None
    #[test]
    fn cross_rate_missing_to_leg_returns_none() {
        let result = cross_rate_micros(Some(1_164_600), None);
        assert_eq!(result, None);
    }

    // A zero `from` leg would divide by zero — guarded to None rather than panicking.
    #[test]
    fn cross_rate_zero_from_leg_returns_none() {
        let result = cross_rate_micros(Some(0), Some(1_000_000));
        assert_eq!(result, None);
    }

    // FXR-082 — truncation toward zero: pick legs whose quotient has a remainder.
    // eur_to_from = 3_000_000 (3.0 in micros), eur_to_to = 1_000_000 (1.0 in micros)
    // rate = 1_000_000 * 1_000_000 / 3_000_000 = 1_000_000_000_000 / 3_000_000 = 333_333 (truncated)
    #[test]
    fn cross_rate_truncates_toward_zero() {
        let result = cross_rate_micros(Some(3_000_000), Some(1_000_000));
        assert_eq!(result, Some(333_333));
    }
}

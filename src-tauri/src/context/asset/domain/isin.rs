//! ISIN format validator (WEB-016).
//!
//! Validates that a raw string conforms to ISO 6166:
//!   1. Trim whitespace and uppercase.
//!   2. Exactly 12 characters.
//!   3. First two chars alphabetic (country code), last char a digit (check digit),
//!      remaining nine chars alphanumeric.
//!   4. Luhn-mod-10 check digit over the digit string produced by expanding letters
//!      to their numeric values (A=10, B=11, …, Z=35) across the first 11 chars.
//!
//! Returns the normalized 12-character uppercase ISIN on success.

/// Domain-level error variants for ISIN format validation (WEB-016).
///
/// Deliberately granular so unit tests can pin the exact rejection reason.
/// The use-case layer maps any variant to the single wire code
/// `InvalidIsinFormat` (WEB-025) — the FE does not need sub-variant granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsinFormatError {
    /// The normalized input is not exactly 12 characters.
    WrongLength,
    /// One or more characters violate the charset rule:
    ///   - first two chars must be ASCII letters (ISO 3166-1 country code)
    ///   - last char must be an ASCII digit (check digit)
    ///   - all remaining chars must be ASCII alphanumeric
    InvalidCharset,
    /// The string length and charset are correct, but the Luhn-mod-10 check
    /// digit does not match.
    BadCheckDigit,
}

/// Validates and normalizes a raw ISIN string (WEB-016).
///
/// Steps:
///   1. Trim leading/trailing ASCII whitespace and uppercase the input.
///   2. Verify length == 12 (`WrongLength`).
///   3. Verify charset: first two chars alphabetic, last char digit, rest
///      alphanumeric (`InvalidCharset`).
///   4. Verify Luhn-mod-10 check digit (`BadCheckDigit`).
///
/// Returns the normalized uppercase 12-character ISIN on success.
pub fn validate_isin(raw: &str) -> Result<String, IsinFormatError> {
    let normalized: String = raw.trim().to_ascii_uppercase();
    if normalized.chars().count() != 12 {
        return Err(IsinFormatError::WrongLength);
    }
    let bytes = normalized.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        let ok = match i {
            0 | 1 => b.is_ascii_uppercase(),
            11 => b.is_ascii_digit(),
            _ => b.is_ascii_uppercase() || b.is_ascii_digit(),
        };
        if !ok {
            return Err(IsinFormatError::InvalidCharset);
        }
    }
    if !luhn_check(&isin_to_digit_string(&normalized)) {
        return Err(IsinFormatError::BadCheckDigit);
    }
    Ok(normalized)
}

// ---------------------------------------------------------------------------
// Private helpers (stubs — to be implemented)
// ---------------------------------------------------------------------------

/// Expands an ISIN string into the digit string used by the Luhn-mod-10
/// algorithm. Each ASCII letter is replaced by its numeric value
/// (A=10, B=11, …, Z=35); digits pass through unchanged.
///
/// The resulting string contains only ASCII digit characters.
fn isin_to_digit_string(isin: &str) -> String {
    let mut out = String::with_capacity(isin.len() * 2);
    for b in isin.bytes() {
        if b.is_ascii_digit() {
            out.push(b as char);
        } else if b.is_ascii_uppercase() {
            let value = (b - b'A') as u32 + 10;
            out.push_str(&value.to_string());
        }
    }
    out
}

/// Runs Luhn-mod-10 validation over a pure-digit string.
///
/// Standard algorithm:
///   - Start from the rightmost digit and double every second digit.
///   - If doubling produces a value >= 10, subtract 9.
///   - Sum all values; valid if the sum mod 10 == 0.
///
/// Returns `true` when the digit string satisfies the check.
fn luhn_check(digits: &str) -> bool {
    let mut sum: u32 = 0;
    for (i, b) in digits.bytes().rev().enumerate() {
        let d = (b - b'0') as u32;
        let v = if i.is_multiple_of(2) {
            d
        } else {
            let doubled = d * 2;
            if doubled >= 10 {
                doubled - 9
            } else {
                doubled
            }
        };
        sum += v;
    }
    sum.is_multiple_of(10)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // WEB-016 — happy path: known-good ISINs
    // ------------------------------------------------------------------

    /// iShares Core S&P 500 UCITS ETF — IE-prefixed, well-known fixture.
    #[test]
    fn validates_ishares_sp500_isin() {
        let result = validate_isin("IE00B53L3W79");
        assert_eq!(result, Ok("IE00B53L3W79".to_string()));
    }

    /// Microsoft — US-prefixed equity.
    #[test]
    fn validates_microsoft_isin() {
        let result = validate_isin("US5949181045");
        assert_eq!(result, Ok("US5949181045".to_string()));
    }

    /// BNP Paribas — FR-prefixed equity.
    #[test]
    fn validates_bnp_paribas_isin() {
        let result = validate_isin("FR0000131104");
        assert_eq!(result, Ok("FR0000131104".to_string()));
    }

    // ------------------------------------------------------------------
    // WEB-016 — happy path: whitespace trim + lowercase upcasing
    // ------------------------------------------------------------------

    /// Leading/trailing spaces are stripped; lowercase is uppercased.
    #[test]
    fn trims_whitespace_and_uppercases() {
        let result = validate_isin("  ie00b53l3w79  ");
        assert_eq!(result, Ok("IE00B53L3W79".to_string()));
    }

    /// Mixed-case input with no surrounding whitespace is also normalized.
    #[test]
    fn uppercases_mixed_case_input() {
        let result = validate_isin("Ie00B53l3W79");
        assert_eq!(result, Ok("IE00B53L3W79".to_string()));
    }

    // ------------------------------------------------------------------
    // WEB-016 — WrongLength
    // ------------------------------------------------------------------

    /// Empty string: 0 chars.
    #[test]
    fn rejects_empty_string_as_wrong_length() {
        assert_eq!(validate_isin(""), Err(IsinFormatError::WrongLength));
    }

    /// 11 characters — one too short.
    #[test]
    fn rejects_eleven_chars_as_wrong_length() {
        assert_eq!(
            validate_isin("IE00B53L3W7"),
            Err(IsinFormatError::WrongLength)
        );
    }

    /// 13 characters — one too long.
    #[test]
    fn rejects_thirteen_chars_as_wrong_length() {
        assert_eq!(
            validate_isin("IE00B53L3W790"),
            Err(IsinFormatError::WrongLength)
        );
    }

    // ------------------------------------------------------------------
    // WEB-016 — InvalidCharset
    // ------------------------------------------------------------------

    /// First character is a digit, not a letter: `1E00B53L3W79`.
    #[test]
    fn rejects_first_char_not_a_letter() {
        assert_eq!(
            validate_isin("1E00B53L3W79"),
            Err(IsinFormatError::InvalidCharset)
        );
    }

    /// Last character is a letter, not a digit: `IE00B53L3W7A`.
    #[test]
    fn rejects_last_char_not_a_digit() {
        assert_eq!(
            validate_isin("IE00B53L3W7A"),
            Err(IsinFormatError::InvalidCharset)
        );
    }

    /// Non-alphanumeric character in the body: `IE00B53-3W79`.
    #[test]
    fn rejects_non_alphanumeric_in_body() {
        assert_eq!(
            validate_isin("IE00B53-3W79"),
            Err(IsinFormatError::InvalidCharset)
        );
    }

    // ------------------------------------------------------------------
    // WEB-016 — BadCheckDigit
    // ------------------------------------------------------------------

    /// `IE00B53L3W70` — last digit mutated from 9 to 0 on a known-good ISIN.
    #[test]
    fn rejects_mutated_check_digit() {
        assert_eq!(
            validate_isin("IE00B53L3W70"),
            Err(IsinFormatError::BadCheckDigit)
        );
    }

    /// `US5949181040` — last digit mutated from 5 to 0 on Microsoft's ISIN.
    #[test]
    fn rejects_mutated_microsoft_check_digit() {
        assert_eq!(
            validate_isin("US5949181040"),
            Err(IsinFormatError::BadCheckDigit)
        );
    }

    // ------------------------------------------------------------------
    // Luhn helper: isin_to_digit_string
    // ------------------------------------------------------------------

    /// `IE00B53L3W79` expands to `18140011532133279`:
    ///   I=18, E=14, 0, 0, B=11, 5, 3, L=21, 3, W=32, 7, 9
    #[test]
    fn expands_isin_to_digit_string_correctly() {
        assert_eq!(isin_to_digit_string("IE00B53L3W79"), "18140011532133279");
    }

    /// Pure-digit string passes through unchanged.
    #[test]
    fn digit_string_passthrough_for_pure_digits() {
        assert_eq!(isin_to_digit_string("123456"), "123456");
    }

    // ------------------------------------------------------------------
    // Luhn helper: luhn_check
    // ------------------------------------------------------------------

    /// `18140011532133279` is the expansion of IE00B53L3W79; Luhn sum must be 0 mod 10.
    #[test]
    fn luhn_check_passes_for_valid_isin_digit_string() {
        assert!(luhn_check("18140011532133279"));
    }

    /// Mutating the last digit by 1 breaks the Luhn check.
    #[test]
    fn luhn_check_fails_for_mutated_digit_string() {
        assert!(!luhn_check("18140011532133270"));
    }
}

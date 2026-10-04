/**
 * Micro-unit conversion utilities (ADR-001, TRX-024).
 *
 * All financial values are stored and transmitted as i64 micro-units (value × 1_000_000).
 * Decimal ↔ micro conversion occurs ONLY at the UI boundary:
 *   - User input:  decimal string → number (micro-units) via decimalToMicro
 *   - Display:     number (micro-units) → formatted decimal string via microToDecimal
 *
 */

const MICRO = 1_000_000;

/**
 * Converts a decimal string to an integer micro-unit value.
 * e.g. "1.5" → 1_500_000
 * Returns 0 for empty, invalid, or non-numeric input.
 *
 * Parses integer and fractional parts separately to avoid IEEE-754 rounding errors.
 */
export function decimalToMicro(value: string): number {
  const trimmed = value.trim().replace(",", ".");
  if (!trimmed || Number.isNaN(Number(trimmed))) return 0;
  const negative = trimmed.startsWith("-");
  const [intStr, fracStr = ""] = trimmed.replace(/^[+-]/, "").split(".");
  const intPart = Number.parseInt(intStr || "0", 10);
  const fracPadded = fracStr.padEnd(6, "0").slice(0, 6);
  const fracPart = Number.parseInt(fracPadded, 10);
  const micros = intPart * MICRO + fracPart;
  return negative ? -micros : micros;
}

/**
 * Converts an integer micro-unit value to a plain decimal string using a period separator.
 * Use for form pre-fill only — not locale-aware.
 * e.g. 1_500_000 → "1.500" (3 decimal places by default per TRX-024)
 */
export function microToDecimal(micros: number, decimals = 3): string {
  return (micros / MICRO).toFixed(decimals);
}

/**
 * Converts a micro-unit value to the decimal string that reads back to the same value:
 * every decimal it carries, none it does not (921_400 → "0.9214", 18_600_000 → "18.6").
 * Use to pre-fill a field from a recorded figure, so saving without typing changes nothing.
 */
export function microToExactDecimal(micros: number): string {
  const text = (micros / MICRO).toFixed(6);
  return text.includes(".") ? text.replace(/0+$/, "").replace(/\.$/, "") : text;
}

// Set once at app startup from i18n config — tests may override via setDisplayLocale("en")
let _displayLocale = "fr";

export function setDisplayLocale(locale: string): void {
  _displayLocale = locale;
}

/**
 * Converts an integer micro-unit value to a locale-aware display string.
 * Use for read-only display in tables and labels — never for editable inputs.
 * Locale follows i18n.language (set at startup via setDisplayLocale).
 * e.g. 1_500_000 → "1,500" (fr) or "1.500" (en) with 3 decimal places
 */
export function microToFormatted(micros: number, decimals = 3): string {
  return new Intl.NumberFormat(_displayLocale, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(micros / MICRO);
}

/**
 * Locale-aware trimmed multiplier for a micro-scaled factor (SPL-060):
 * 20_000_000 → "20", 1_500_000 → "1.5" (en) / "1,5" (fr), 100_000 → "0.1".
 */
export function microToFormattedFactor(factorMicros: number): string {
  return new Intl.NumberFormat(_displayLocale, {
    maximumFractionDigits: 6,
  }).format(factorMicros / MICRO);
}

/**
 * Converts a price in micro-units to a locale-aware display string with adaptive
 * precision: 3 decimal places when the absolute value is below 10, 2 otherwise.
 * e.g. 7_500_000 → "7.500", 150_000_000 → "150.00"
 */
export function microToFormattedPrice(micros: number): string {
  const decimals = Math.abs(micros) < 10 * MICRO ? 3 : 2;
  return microToFormatted(micros, decimals);
}

/**
 * Converts a quantity in micro-units to a locale-aware display string, trimming
 * trailing zero decimals: a whole number shows no fraction, otherwise up to 6
 * fractional digits are kept.
 * e.g. 2_000_000 → "2", 1_500_000 → "1.5", 1_250_000 → "1.25"
 */
export function microToFormattedQuantity(micros: number): string {
  return new Intl.NumberFormat(_displayLocale, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 6,
  }).format(micros / MICRO);
}

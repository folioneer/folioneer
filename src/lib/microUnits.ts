/**
 * Micro-unit conversion utilities (ADR-001, TRX-024).
 *
 * All financial values are stored and transmitted as i64 micro-units (value × 1_000_000).
 * Decimal ↔ micro conversion occurs ONLY at the UI boundary:
 *   - User input:  decimal string → number (micro-units) via decimalToMicro
 *   - Display:     number (micro-units) → formatted decimal string via microToDecimal
 *
 * All internal calculations are performed on micro-unit integers (computeTotalMicro).
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

/**
 * Computes total amount from micro-unit values (TRX-026 formula).
 * Formula: floor(floor(qty × price / MICRO) × rate / MICRO) + fees
 *
 * All arguments and the return value are in micro-units.
 * Mirrors the backend integer arithmetic exactly — no decimal conversion involved.
 */
export function computeTotalMicro(
  qtyMicro: number,
  priceMicro: number,
  rateMicro: number,
  feesMicro: number,
): number {
  return Math.floor((Math.floor((qtyMicro * priceMicro) / MICRO) * rateMicro) / MICRO) + feesMicro;
}

/**
 * Computes the VWAP cost basis of a quantity (the account-currency cost of
 * `qtyMicro` units at `avgPriceMicro` per unit): floor(avgPrice × qty / MICRO).
 * Mirrors the backend realized-P&L cost term (SEL-024 / TDI-030).
 */
export function computeCostBasisMicro(avgPriceMicro: number, qtyMicro: number): number {
  return Math.floor((avgPriceMicro * qtyMicro) / MICRO);
}

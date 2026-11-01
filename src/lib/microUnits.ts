import { isCashAsset } from "./cashAsset";

/**
 * Micro-unit conversion utilities (ADR-001, TRX-024).
 *
 * All financial values are stored and transmitted as i64 micro-units (value × 1_000_000).
 * A quantity is the exception: it crosses as decimal text (TRX-024B), and sums and comparisons
 * of quantities go through the helpers below, never through a number.
 * Decimal ↔ micro conversion occurs ONLY at the UI boundary:
 *   - User input:  decimal string → number (micro-units) via decimalToMicro
 *   - A form field: number (micro-units) → decimal string via microToFieldDecimal
 *
 */

const MICRO = 1_000_000;

/**
 * Converts a decimal string, written with a dot, to an integer micro-unit value.
 * e.g. "1.5" → 1_500_000
 * Returns 0 for empty, invalid, or non-numeric input: a comma is not a number here —
 * what a field shows in the display language is read by `typedToDecimal` first (NUM-010).
 *
 * Parses integer and fractional parts separately to avoid IEEE-754 rounding errors.
 */
export function decimalToMicro(value: string): number {
  const trimmed = value.trim();
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
 * Converts a micro-unit value to the decimal string that reads back to the same value:
 * every decimal it carries, none it does not (921_400 → "0.9214", 18_600_000 → "18.6").
 * Use to pre-fill a field from a recorded figure, so saving without typing changes nothing.
 */
export function microToExactDecimal(micros: number): string {
  const text = (micros / MICRO).toFixed(6);
  return text.includes(".") ? text.replace(/0+$/, "").replace(/\.$/, "") : text;
}

/**
 * As `microToExactDecimal`, with at least three decimals, the look of a figure in a form
 * (100_000_000 → "100.000", 921_400 → "0.9214", 123_456 → "0.123456"). Use to fill a form
 * field from a recorded or computed figure: saving it untouched changes nothing (TRX-024).
 */
export function microToFieldDecimal(micros: number): string {
  const [whole, decimals = ""] = microToExactDecimal(micros).split(".");
  return `${whole}.${decimals.padEnd(3, "0")}`;
}

// Follows the display language: set from the i18n config at start-up and on every change
// of language — tests may override via setDisplayLocale("en").
let _displayLocale = "fr";

export function setDisplayLocale(locale: string): void {
  _displayLocale = locale;
}

/** NUM-001 — the decimal separator of the display language: a comma in French, a dot otherwise. */
function decimalSeparator(): "," | "." {
  return _displayLocale.toLowerCase().startsWith("fr") ? "," : ".";
}

/**
 * NUM-010 — reads what is typed in a number field: in French a comma or a dot is the decimal
 * separator; in English only the dot is, and a comma is left as typed, which is no number.
 * The result is written with a dot, the form in which every figure travels.
 */
export function typedToDecimal(typed: string): string {
  return decimalSeparator() === "," ? typed.replaceAll(",", ".") : typed;
}

/**
 * NUM-011 — writes a figure in a number field, from its dot form: with a comma in French,
 * as it is in English.
 */
export function decimalToDisplayed(decimal: string): string {
  return decimalSeparator() === "," ? decimal.replaceAll(".", ",") : decimal;
}

/**
 * Reads a decimal text written with a dot as a number; NaN when it is not one. Unlike
 * `parseFloat`, it reads nothing out of a text that only starts like a number ("1,5").
 */
export function decimalToNumber(decimal: string): number {
  const trimmed = decimal.trim();
  return trimmed === "" ? Number.NaN : Number(trimmed);
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

/** TRX-024B — the decimals a quantity of a non-cash asset carries. */
const QUANTITY_DECIMALS = 8;

/** TRX-024B — the decimals of a quantity of a Cash Asset, a split's factor, a dividend's amount. */
export const MONEY_DECIMALS = 6;

/** TRX-024B — the decimals a quantity of `assetId` carries. */
export function decimalsOf(assetId: string): number {
  return isCashAsset(assetId) ? MONEY_DECIMALS : QUANTITY_DECIMALS;
}

/**
 * TRX-024H — what a form sends as a quantity: `value`, written with a dot, kept to `decimals`
 * decimals as `decimalToMicro` keeps six, without trailing zeros; "0" for what is not a number.
 * e.g. "1.50" → "1.5", "0.123456789" → "0.12345678"
 */
export function decimalToQuantity(value: string, decimals = QUANTITY_DECIMALS): string {
  const trimmed = value.trim();
  if (!/^[+-]?(\d+\.?\d*|\.\d+)$/.test(trimmed)) return "0";
  const negative = trimmed.startsWith("-");
  const [whole = "", fraction = ""] = trimmed.replace(/^[+-]/, "").split(".");
  const kept = fraction.slice(0, decimals).replace(/0+$/, "");
  const digits = (whole.replace(/^0+(?=\d)/, "") || "0") + (kept ? `.${kept}` : "");
  return negative && digits !== "0" ? `-${digits}` : digits;
}

/** A quantity's decimal text as a whole number of hundred-millionths, for exact sums and comparisons. */
function quantitySteps(quantity: string): bigint {
  const normal = decimalToQuantity(quantity);
  const negative = normal.startsWith("-");
  const [whole = "0", fraction = ""] = normal.replace("-", "").split(".");
  const steps = BigInt(whole + fraction.padEnd(QUANTITY_DECIMALS, "0"));
  return negative ? -steps : steps;
}

/** A whole number of hundred-millionths as a quantity's decimal text. */
function stepsToQuantity(steps: bigint): string {
  const negative = steps < 0n;
  const digits = (negative ? -steps : steps).toString().padStart(QUANTITY_DECIMALS + 1, "0");
  const whole = digits.slice(0, -QUANTITY_DECIMALS);
  const fraction = digits.slice(-QUANTITY_DECIMALS).replace(/0+$/, "");
  return `${negative ? "-" : ""}${whole}${fraction ? `.${fraction}` : ""}`;
}

/** Negative, zero or positive as `a` is below, equal to or above `b`, exactly. */
export function compareQuantities(a: string, b: string): number {
  const difference = quantitySteps(a) - quantitySteps(b);
  if (difference === 0n) return 0;
  return difference < 0n ? -1 : 1;
}

/** The sum of two quantities, exactly. */
export function addQuantities(a: string, b: string): string {
  return stepsToQuantity(quantitySteps(a) + quantitySteps(b));
}

/** `a − b`, exactly. */
export function subtractQuantities(a: string, b: string): string {
  return stepsToQuantity(quantitySteps(a) - quantitySteps(b));
}

/** What `quantity` costs at `priceMicros` a unit, in micro-units, rounded toward zero. */
export function quantityCost(quantity: string, priceMicros: number): number {
  return Number((quantitySteps(quantity) * BigInt(priceMicros)) / 10n ** BigInt(QUANTITY_DECIMALS));
}

/** Whether a quantity is above zero. */
export function isPositiveQuantity(quantity: string): boolean {
  return quantitySteps(quantity) > 0n;
}

/**
 * TRX-024B — the unit cost of `totalMicros` over `quantity`, in micro-units, rounded toward zero;
 * 0 for no quantity.
 */
export function microPerUnit(totalMicros: number, quantity: string): number {
  const steps = quantitySteps(quantity);
  if (steps <= 0n) return 0;
  return Number((BigInt(totalMicros) * 10n ** BigInt(QUANTITY_DECIMALS)) / steps);
}

/**
 * TRX-024B — a quantity in a form field: every decimal it carries and at least three, as
 * `microToFieldDecimal` writes a figure (100 → "100.000", 0.00000001 → "0.00000001").
 */
export function quantityToFieldDecimal(quantity: string): string {
  const [whole = "0", decimals = ""] = quantity.split(".");
  return `${whole}.${decimals.padEnd(3, "0")}`;
}

/**
 * TRX-024I — a quantity to read, in the display language: trimmed, up to eight decimals by
 * default ("2" → "2", "1.5" → "1.5"), or with at least `minimumDecimals` in a journal row.
 */
export function formatQuantity(quantity: string, minimumDecimals = 0): string {
  return new Intl.NumberFormat(_displayLocale, {
    minimumFractionDigits: minimumDecimals,
    maximumFractionDigits: QUANTITY_DECIMALS,
  }).format(quantity as Intl.StringNumericLiteral);
}

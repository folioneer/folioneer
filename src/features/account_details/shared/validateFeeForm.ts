import { decimalToNumber } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";
import { validateDate } from "./validateCashForm";

/**
 * Validation helpers for the management-fee forms — the one-off deduction
 * (FEE-021) and the recurring schedule (FEE-032). Returns an I18nMessage on
 * failure, or null when the value is acceptable. The backend re-validates every
 * field, so these mirror its bounds: a percentage strictly positive, at most 100%
 * for a one-off deduction and strictly below 100% for a schedule, whose end date
 * is strictly after its start date.
 */
export function validatePercentage(percent: string): I18nMessage | null {
  if (percent.length === 0) return { key: "validation.percentage_not_positive" };
  const value = decimalToNumber(percent);
  if (!Number.isFinite(value) || value <= 0) return { key: "validation.percentage_not_positive" };
  if (value > 100) return { key: "validation.percentage_above_hundred" };
  return null;
}

/**
 * FEE-021 — the quantity a holding should hold after a one-off fee: a number, zero or
 * more. The backend alone knows the quantity held as of the fee's date, so the upper
 * bound is its rejection (FEE-028/029), not a check here.
 */
export function validateResultingQuantity(quantity: string): I18nMessage | null {
  const value = decimalToNumber(quantity);
  if (!Number.isFinite(value) || value < 0) return { key: "validation.resulting_quantity_invalid" };
  return null;
}

/**
 * FEE-032 — a schedule needs a valid rate strictly below 100 %, a valid start date, and (when present)
 * an end date that is valid and strictly after the start date. The first failing
 * field wins so the modal surfaces one message at a time.
 */
export function validateFeeSchedule(fields: {
  ratePercent: string;
  startDate: string;
  endDate: string;
}): I18nMessage | null {
  const rateErr = validatePercentage(fields.ratePercent);
  if (rateErr) return rateErr;
  // FEE-032 — a schedule's rate stays strictly below 100 % a year.
  if (decimalToNumber(fields.ratePercent) >= 100) return { key: "error.RateAboveHundred" };
  const startErr = validateDate(fields.startDate);
  if (startErr) return startErr;
  if (fields.endDate.length > 0) {
    const endErr = validateDate(fields.endDate);
    if (endErr) return endErr;
    if (fields.endDate <= fields.startDate) return { key: "validation.end_date_before_start" };
  }
  return null;
}

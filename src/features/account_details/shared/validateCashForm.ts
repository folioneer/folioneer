import type { I18nMessage } from "@/ui/format/i18n";

/**
 * Validation helpers for Deposit / Withdrawal forms (CSH-021, CSH-031).
 * Returns an I18nMessage on failure, or null when the value is acceptable.
 */
export function validateAmount(amount: string): I18nMessage | null {
  if (amount.length === 0) return { key: "validation.amount_not_positive" };
  const n = parseFloat(amount);
  if (!Number.isFinite(n) || n <= 0) return { key: "validation.amount_not_positive" };
  return null;
}

export function validateDate(date: string): I18nMessage | null {
  if (date.length === 0 || !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    return { key: "validation.invalid_date" };
  }
  const today = new Date().toISOString().slice(0, 10);
  if (date > today) return { key: "validation.date_in_future" };
  if (date < "1900-01-01") return { key: "validation.date_too_old" };
  return null;
}

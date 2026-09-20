import { microToFormatted } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";
import type { TransactionFormData } from "./types";

/**
 * Client-side validation for the transaction form (mirrors TRX-020).
 * Receives already-computed micro-unit values to avoid redundant conversions.
 * Returns the first error message, or null if the form is valid.
 */
export function validateTransactionForm(
  data: TransactionFormData,
  qtyMicro: number,
  totalMicro: number,
  totalEntryFeesMicro: number | null = null,
): I18nMessage | null {
  if (!data.accountId) return { key: "transaction.error_validation_account" };
  if (!data.assetId) return { key: "transaction.error_validation_asset" };
  if (!data.date) return { key: "transaction.error_validation_date" };
  if (qtyMicro <= 0) return { key: "transaction.error_validation_quantity" };
  if (totalMicro <= 0) return { key: "transaction.error_validation_total" };
  // TRX-060 — total-entry buy: the all-in total includes the fees, so the
  // securities part (total − fees) must not be negative. Pass the fees only
  // in total-entry purchase mode; null skips the check.
  if (totalEntryFeesMicro !== null && totalMicro < totalEntryFeesMicro) {
    return { key: "transaction.error_validation_total_below_fees" };
  }
  return null;
}

/**
 * Client-side validation for the sell transaction form (SEL-022).
 * Same base rules as purchase, plus oversell guard against maxQuantityMicro.
 * Returns the first error message, or null if the form is valid.
 */
export function validateSellForm(
  data: TransactionFormData,
  qtyMicro: number,
  totalMicro: number,
  maxQuantityMicro: number,
): I18nMessage | null {
  const base = validateTransactionForm(data, qtyMicro, totalMicro);
  if (base) return base;
  if (qtyMicro > maxQuantityMicro) {
    // reviewer-arch FP: validator owns vars assembly when the value is a call-site arg
    // (holding quantity passed in by the caller), not a backend-typed payload — keeping
    // microToFormatted here avoids threading raw micros through a separate presenter.
    return {
      key: "transaction.error_validation_oversell",
      vars: { max: microToFormatted(maxQuantityMicro, 6) },
    };
  }
  return null;
}

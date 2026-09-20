import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { interestErrorToI18n } from "../shared/presenter";
import { validateAmount, validateDate } from "../shared/validateCashForm";
import { validatePercentage } from "../shared/validateFeeForm";

/** A holding interest can be credited to — active non-cash or the cash line (INT-011/020/023). */
export interface InterestHolding {
  assetId: string;
  assetName: string;
  assetCurrency: string;
}

/** Edit-mode context (INT-040): the transaction being corrected; the asset is immutable. */
export interface InterestEditMode {
  transactionId: string;
  lockedAssetId: string;
  lockedAssetName: string;
  /** Current values to prefill the form when correcting an existing credit. */
  initialDate?: string;
  initialQuantity?: string;
  initialNote?: string;
}

interface UseInterestTransactionProps {
  accountId: string;
  onSubmitSuccess?: () => void;
  editMode?: InterestEditMode;
}

interface InterestFormData {
  assetId: string;
  date: string;
  percent: string;
  quantity: string;
  note: string;
}

/**
 * INT-021 — exactly one of percent / quantity must be filled; both or neither
 * fails, otherwise the filled field's own numeric validation applies.
 */
function validateInterestAmount(percent: string, quantity: string): I18nMessage | null {
  const exactlyOne = (percent !== "") !== (quantity !== "");
  if (!exactlyOne) return { key: "error.InterestAmountInvalid" };
  return percent !== "" ? validatePercentage(percent) : validateAmount(quantity);
}

export function useInterestTransaction({
  accountId,
  onSubmitSuccess,
  editMode,
}: UseInterestTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  // INT-040 — in edit mode the credited asset is fixed.
  const isAssetLocked = editMode != null;

  const [formData, setFormData] = useState<InterestFormData>(() => ({
    assetId: editMode?.lockedAssetId ?? "",
    date: editMode?.initialDate ?? getLastOperationDate(accountId),
    percent: "",
    quantity: editMode?.initialQuantity ?? "",
    note: editMode?.initialNote ?? "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // INT-021 — the submit gate requires an asset, a valid date, and at least one
  // amount field; the exactly-one check is left to handleSubmit so a user who
  // filled both fields gets the InterestAmountInvalid message instead of a
  // silently inert button.
  const isFormValid = useMemo(
    () =>
      formData.assetId !== "" &&
      (formData.percent.trim() !== "" || formData.quantity.trim() !== "") &&
      validateDate(formData.date) === null,
    [formData.assetId, formData.percent, formData.quantity, formData.date],
  );

  const handleChange = useCallback(
    (field: keyof InterestFormData, value: string) => {
      // The asset is immutable in edit mode (INT-040) — ignore selector changes.
      if (field === "assetId" && isAssetLocked) return;
      setFormData((prev) => ({ ...prev, [field]: value }));
    },
    [isAssetLocked],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const amountErr = validateInterestAmount(formData.percent, formData.quantity);
      const dateErr = validateDate(formData.date);
      const validationError =
        formData.assetId === "" ? { key: "error.AssetNotHeld" } : (amountErr ?? dateErr);
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        const note = formData.note || null;

        const result = editMode
          ? // INT-040 — edit reuses correct_transaction; the Interest branch on the
            // backend re-packs the zero-cost convention, so the money fields it carries
            // are inert (unit_price 0, exchange_rate 1.0, fees 0).
            await accountDetailsGateway.correctTransaction(editMode.transactionId, accountId, {
              date: formData.date,
              quantity: decimalToMicro(formData.quantity),
              unit_price: 0,
              exchange_rate: 1_000_000,
              fees: 0,
              total_amount: null,
              note,
            })
          : // INT-021 — 1% = 1_000_000 micro-percent, the same micro scaling as decimals.
            await accountDetailsGateway.recordInterest({
              account_id: accountId,
              asset_id: formData.assetId,
              date: formData.date,
              percent_micros: formData.percent !== "" ? decimalToMicro(formData.percent) : null,
              quantity_micros: formData.quantity !== "" ? decimalToMicro(formData.quantity) : null,
              note,
            });

        if (result.status === "error") {
          logger.error("[useInterestTransaction] recordInterest failed", {
            error: result.error,
          });
          setError(interestErrorToI18n(result.error));
          return;
        }
        if (!editMode) setLastOperationDate(accountId, formData.date);
        showSnackbar(t(editMode ? "interest.updated" : "interest.recorded"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [accountId, formData, editMode, t, showSnackbar, onSubmitSuccess],
  );

  return {
    formData,
    error,
    isSubmitting,
    isFormValid,
    isAssetLocked,
    handleChange,
    handleSubmit,
  };
}

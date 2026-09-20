import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { freeSharesErrorToI18n } from "../shared/presenter";
import { validateAmount, validateDate } from "../shared/validateCashForm";

/** A holding the free shares can be attributed to — active, non-cash (FSD-011/020). */
export interface FreeSharesHolding {
  assetId: string;
  assetName: string;
  assetCurrency: string;
}

/** Edit-mode context (FSD-040): the transaction being corrected; the asset is immutable. */
export interface FreeSharesEditMode {
  transactionId: string;
  lockedAssetId: string;
  lockedAssetName: string;
  /** Current values to prefill the form when correcting an existing distribution. */
  initialDate?: string;
  initialQuantity?: string;
  initialNote?: string;
}

interface UseFreeSharesTransactionProps {
  accountId: string;
  onSubmitSuccess?: () => void;
  editMode?: FreeSharesEditMode;
}

interface FreeSharesFormData {
  assetId: string;
  date: string;
  quantity: string;
  note: string;
}

export function useFreeSharesTransaction({
  accountId,
  onSubmitSuccess,
  editMode,
}: UseFreeSharesTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  // FSD-040 — in edit mode the distributing asset is fixed.
  const isAssetLocked = editMode != null;

  const [formData, setFormData] = useState<FreeSharesFormData>(() => ({
    assetId: editMode?.lockedAssetId ?? "",
    date: editMode?.initialDate ?? getLastOperationDate(accountId),
    quantity: editMode?.initialQuantity ?? "",
    note: editMode?.initialNote ?? "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // FSD-021 — asset selected, quantity strictly positive, date valid.
  const isFormValid = useMemo(
    () =>
      formData.assetId !== "" &&
      validateAmount(formData.quantity) === null &&
      validateDate(formData.date) === null,
    [formData.assetId, formData.quantity, formData.date],
  );

  const handleChange = useCallback(
    (field: keyof FreeSharesFormData, value: string) => {
      // The asset is immutable in edit mode (FSD-040) — ignore selector changes.
      if (field === "assetId" && isAssetLocked) return;
      setFormData((prev) => ({ ...prev, [field]: value }));
    },
    [isAssetLocked],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const quantityErr = validateAmount(formData.quantity);
      const dateErr = validateDate(formData.date);
      const validationError =
        formData.assetId === "" ? { key: "error.AssetNotHeld" } : (quantityErr ?? dateErr);
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        const quantityMicros = decimalToMicro(formData.quantity);
        const note = formData.note || null;

        const result = editMode
          ? // FSD-040 — edit reuses correct_transaction; the FreeShares branch on the
            // backend re-packs the zero-cost convention, so the money fields it carries
            // are inert (unit_price 0, exchange_rate 1.0, fees 0).
            await accountDetailsGateway.correctTransaction(editMode.transactionId, accountId, {
              date: formData.date,
              quantity: quantityMicros,
              unit_price: 0,
              exchange_rate: 1_000_000,
              fees: 0,
              total_amount: null,
              note,
            })
          : await accountDetailsGateway.recordFreeShares({
              account_id: accountId,
              asset_id: formData.assetId,
              date: formData.date,
              quantity: quantityMicros,
              note,
            });

        if (result.status === "error") {
          logger.error("[useFreeSharesTransaction] recordFreeShares failed", {
            error: result.error,
          });
          setError(freeSharesErrorToI18n(result.error));
          return;
        }
        if (!editMode) setLastOperationDate(accountId, formData.date);
        showSnackbar(t("free_shares.recorded"), "success");
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

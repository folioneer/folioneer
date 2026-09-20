import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { managementFeeErrorToI18n } from "../shared/presenter";
import { validateAmount, validateDate } from "../shared/validateCashForm";

/**
 * Edit-mode context (FEE-063): the fee deduction being corrected. The charged
 * asset is immutable; the entry-time percentage is not retained, so the edit
 * surface adjusts the stored removed `quantity` directly.
 */
export interface ManagementFeeEditContext {
  transactionId: string;
  lockedAssetName: string;
  initialDate: string;
  initialQuantity: string;
  initialNote: string;
}

interface UseManagementFeeEditProps {
  accountId: string;
  editContext: ManagementFeeEditContext;
  onSubmitSuccess?: () => void;
}

interface ManagementFeeEditFormData {
  date: string;
  quantity: string;
  note: string;
}

export function useManagementFeeEdit({
  accountId,
  editContext,
  onSubmitSuccess,
}: UseManagementFeeEditProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<ManagementFeeEditFormData>(() => ({
    date: editContext.initialDate,
    quantity: editContext.initialQuantity,
    note: editContext.initialNote,
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // FEE-063 — removed quantity strictly positive, date valid.
  const isFormValid = useMemo(
    () => validateAmount(formData.quantity) === null && validateDate(formData.date) === null,
    [formData.quantity, formData.date],
  );

  const handleChange = useCallback((field: keyof ManagementFeeEditFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const validationError = validateAmount(formData.quantity) ?? validateDate(formData.date);
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        // FEE-063 — edit reuses correct_transaction; the ManagementFee branch on the
        // backend re-packs the zero-cost convention, so the money fields it carries
        // are inert (unit_price 0, exchange_rate 1.0, fees 0).
        const result = await accountDetailsGateway.correctTransaction(
          editContext.transactionId,
          accountId,
          {
            date: formData.date,
            quantity: decimalToMicro(formData.quantity),
            unit_price: 0,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: null,
            note: formData.note || null,
          },
        );

        if (result.status === "error") {
          logger.error("[useManagementFeeEdit] correctTransaction failed", { error: result.error });
          setError(managementFeeErrorToI18n(result.error));
          return;
        }
        showSnackbar(t("management_fee.updated"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [accountId, formData, editContext.transactionId, t, showSnackbar, onSubmitSuccess],
  );

  return {
    formData,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
  };
}

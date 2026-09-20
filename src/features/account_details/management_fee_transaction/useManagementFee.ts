import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ManagementFeeRemoval } from "@/bindings";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { managementFeeErrorToI18n } from "../shared/presenter";
import { validateDate } from "../shared/validateCashForm";
import { validatePercentage, validateResultingQuantity } from "../shared/validateFeeForm";

/** A holding a one-off management fee can be charged against — active, non-cash (FEE-011/012). */
export interface ManagementFeeHolding {
  assetId: string;
  assetName: string;
  assetCurrency: string;
}

/** FEE-020 — how the fee is entered: a percentage of the holding, or the quantity it should end at. */
export type ManagementFeeMode = "percent" | "quantity";

interface UseManagementFeeProps {
  accountId: string;
  onSubmitSuccess?: () => void;
}

interface ManagementFeeFormData {
  assetId: string;
  date: string;
  mode: ManagementFeeMode;
  percent: string;
  resultingQuantity: string;
  note: string;
}

type ManagementFeeTextField = Exclude<keyof ManagementFeeFormData, "mode">;

export function useManagementFee({ accountId, onSubmitSuccess }: UseManagementFeeProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<ManagementFeeFormData>(() => ({
    assetId: "",
    date: getLastOperationDate(accountId),
    mode: "percent",
    percent: "",
    resultingQuantity: "",
    note: "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [preview, setPreview] = useState<ManagementFeeRemoval | null>(null);
  const [previewError, setPreviewError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { assetId, date, mode, resultingQuantity } = formData;

  // FEE-029 — in quantity mode the backend says what the entry removes; the preview is
  // dropped as soon as an input changes, so a stale one can never validate the form.
  // reviewer-frontend FP: no loading flag — the preview is a local replay answering in
  // milliseconds, and the disabled submit button already marks the wait.
  useEffect(() => {
    setPreview(null);
    setPreviewError(null);
    if (
      mode !== "quantity" ||
      assetId === "" ||
      validateDate(date) !== null ||
      validateResultingQuantity(resultingQuantity) !== null
    ) {
      return;
    }
    let isCurrent = true;
    accountDetailsGateway
      .previewManagementFee(accountId, assetId, date, decimalToMicro(resultingQuantity))
      .then((result) => {
        if (!isCurrent) return;
        if (result.status === "ok") {
          setPreview(result.data);
        } else {
          setPreviewError(managementFeeErrorToI18n(result.error));
        }
      });
    return () => {
      isCurrent = false;
    };
  }, [accountId, assetId, date, mode, resultingQuantity]);

  // FEE-021 — asset selected, date valid, and either a percentage in (0, 100] or a
  // resulting quantity the backend accepted (FEE-029).
  const isFormValid = useMemo(
    () =>
      formData.assetId !== "" &&
      validateDate(formData.date) === null &&
      (formData.mode === "percent"
        ? validatePercentage(formData.percent) === null
        : preview !== null),
    [formData.assetId, formData.date, formData.mode, formData.percent, preview],
  );

  const handleChange = useCallback((field: ManagementFeeTextField, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handleModeChange = useCallback((nextMode: ManagementFeeMode) => {
    setFormData((prev) => ({ ...prev, mode: nextMode }));
    // A message about the other mode's input no longer applies.
    setError(null);
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const amountErr =
        formData.mode === "percent"
          ? validatePercentage(formData.percent)
          : validateResultingQuantity(formData.resultingQuantity);
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
        // 1% = 1_000_000 micro-percent — the same micro scaling as decimals (FEE-021).
        const result = await accountDetailsGateway.recordManagementFee({
          account_id: accountId,
          asset_id: formData.assetId,
          date: formData.date,
          percent_micros: formData.mode === "percent" ? decimalToMicro(formData.percent) : null,
          resulting_quantity_micros:
            formData.mode === "quantity" ? decimalToMicro(formData.resultingQuantity) : null,
          note: formData.note || null,
        });

        if (result.status === "error") {
          logger.error("[useManagementFee] recordManagementFee failed", { error: result.error });
          setError(managementFeeErrorToI18n(result.error));
          return;
        }
        setLastOperationDate(accountId, formData.date);
        showSnackbar(t("management_fee.recorded"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [accountId, formData, t, showSnackbar, onSubmitSuccess],
  );

  return {
    formData,
    error,
    preview,
    previewError,
    isSubmitting,
    isFormValid,
    handleChange,
    handleModeChange,
    handleSubmit,
  };
}

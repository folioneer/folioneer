import type React from "react";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { OpeningBalanceDraft } from "@/bindings";
import { transactionMutationErrorToI18n } from "@/features/transactions/shared/presenter";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { todayIso } from "@/ui/format/date";
import type { I18nMessage } from "@/ui/format/i18n";
import { useLatestCheck } from "@/ui/hooks/useLatestCheck";
import { accountDetailsGateway } from "../gateway";
import { transactionDraftErrorToI18n } from "../shared/presenter";
import { type DraftField, useDraftProblemDisplay } from "../shared/useDraftProblemDisplay";

interface UseOpenBalanceProps {
  accountId: string;
  assetId: string;
  onSubmitSuccess?: () => void;
}

export interface OpenBalanceFormData {
  accountId: string;
  assetId: string;
  date: string;
  quantity: string;
  totalCost: string;
}

export function useOpenBalance({ accountId, assetId, onSubmitSuccess }: UseOpenBalanceProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<OpenBalanceFormData>(() => ({
    accountId,
    assetId,
    date: todayIso(),
    quantity: "",
    totalCost: "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // TRX-066 — the core checks the draft each time it changes: saving is enabled once the
  // latest draft checks clean, and the core says whether a zero cost calls for the warning
  // (TRX-065). A total cost not typed yet is sent as none, never as 0.
  const draft = useMemo<OpeningBalanceDraft>(
    () => ({
      account_id: formData.accountId,
      asset_id: formData.assetId,
      date: formData.date,
      quantity: decimalToMicro(formData.quantity),
      total_cost: formData.totalCost.trim() === "" ? null : decimalToMicro(formData.totalCost),
    }),
    [formData],
  );
  const logCheckFailure = useCallback(
    (cause: unknown) =>
      logger.error("[useOpenBalance] opening balance draft check failed", { error: cause }),
    [],
  );
  const check = useLatestCheck(
    draft,
    accountDetailsGateway.validateOpeningBalanceDraft,
    logCheckFailure,
  );
  const isFormValid = check.data !== null;
  const zeroCostWarning = check.data?.zero_cost ?? false;
  // TRX-067 — the first problem is shown on its field once typed in, as a hint before; a
  // check that could not run is said beside the actions.
  const problemMessage = useMemo<I18nMessage | null>(() => {
    if (check.failed) return { key: "error.Unknown" };
    return check.error ? transactionDraftErrorToI18n(check.error) : null;
  }, [check.failed, check.error]);
  const problemDisplay = useDraftProblemDisplay(check.error, problemMessage);
  const touch = problemDisplay.touch;

  const handleChange = useCallback(
    (field: keyof OpenBalanceFormData, value: string) => {
      setFormData((prev) => ({ ...prev, [field]: value }));
      const typed: Partial<Record<keyof OpenBalanceFormData, DraftField>> = {
        date: "date",
        quantity: "quantity",
        totalCost: "totalCost",
      };
      const draftField = typed[field];
      if (draftField) touch(draftField);
    },
    [touch],
  );

  const handleSubmit = useCallback(
    async (e: React.SyntheticEvent) => {
      e.preventDefault();
      setError(null);
      setIsSubmitting(true);
      try {
        const result = await accountDetailsGateway.openHolding({
          account_id: formData.accountId,
          asset_id: formData.assetId,
          date: formData.date,
          quantity: draft.quantity,
          total_cost: draft.total_cost ?? 0,
        });
        if (result.status === "ok") {
          showSnackbar(t("open_balance.success_created"), "success");
          onSubmitSuccess?.();
        } else {
          logger.error("[useOpenBalance] openHolding failed", {
            error: result.error,
          });
          setError(transactionMutationErrorToI18n(result.error));
        }
      } finally {
        setIsSubmitting(false);
      }
    },
    [formData, draft, onSubmitSuccess, showSnackbar, t],
  );

  return {
    formData,
    error: error ?? problemDisplay.alert,
    /** TRX-067 — the first problem as an error on the field it concerns, once typed in. */
    fieldErrors: problemDisplay.fieldErrors,
    /** TRX-067 — what to enter, for a field not typed in yet. */
    problemHint: problemDisplay.hint,
    isSubmitting,
    isFormValid,
    zeroCostWarning,
    handleChange,
    handleSubmit,
  };
}

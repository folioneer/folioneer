import type React from "react";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { transactionMutationErrorToI18n } from "@/features/transactions/shared/presenter";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";

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
    date: new Date().toISOString().slice(0, 10),
    quantity: "",
    totalCost: "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isFormValid = useMemo(() => {
    const qty = parseFloat(formData.quantity);
    const cost = parseFloat(formData.totalCost);
    const today = new Date().toISOString().slice(0, 10);
    // TRX-046: date must not be in the future. TRX-045: total cost may be 0 (a
    // zero-cost position — mined / gifted / airdropped); empty (NaN) stays invalid.
    return !!formData.assetId && !!formData.date && formData.date <= today && qty > 0 && cost >= 0;
  }, [formData.assetId, formData.date, formData.quantity, formData.totalCost]);

  const handleChange = useCallback((field: keyof OpenBalanceFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

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
          quantity: decimalToMicro(formData.quantity),
          total_cost: decimalToMicro(formData.totalCost),
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
    [formData, onSubmitSuccess, showSnackbar, t],
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

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { FeeFrequency } from "@/bindings";
import { logger } from "@/lib/logger";
import { decimalToMicro, microToDecimal } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { managementFeeErrorToI18n } from "../shared/presenter";
import { validateFeeSchedule } from "../shared/validateFeeForm";

interface UseFeeScheduleProps {
  accountId: string;
  assetId: string;
  onSubmitSuccess?: () => void;
}

interface FeeScheduleFormData {
  ratePercent: string;
  frequency: FeeFrequency;
  startDate: string;
  endDate: string;
  active: boolean;
}

const todayIso = (): string => new Date().toISOString().slice(0, 10);

export function useFeeSchedule({ accountId, assetId, onSubmitSuccess }: UseFeeScheduleProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<FeeScheduleFormData>(() => ({
    ratePercent: "",
    frequency: "Monthly",
    startDate: todayIso(),
    endDate: "",
    active: true,
  }));
  // FEE-060 — an existing schedule switches the form to update mode (frequency and
  // start_date become immutable) and reveals the Delete action.
  const [isExisting, setIsExisting] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // FEE-030 — load the current schedule (if any) to prefill the form.
  useEffect(() => {
    let mounted = true;
    accountDetailsGateway
      .getFeeSchedule(accountId, assetId)
      .then((result) => {
        if (!mounted) return;
        if (result.status === "error") {
          logger.error("[useFeeSchedule] getFeeSchedule failed", { error: result.error });
          setError(managementFeeErrorToI18n(result.error));
          return;
        }
        const schedule = result.data;
        if (schedule) {
          setIsExisting(true);
          setFormData({
            ratePercent: microToDecimal(schedule.annual_rate_percent_micros, 3),
            frequency: schedule.frequency,
            startDate: schedule.start_date,
            endDate: schedule.end_date ?? "",
            active: schedule.active,
          });
        }
      })
      .finally(() => {
        if (mounted) setIsLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [accountId, assetId]);

  const isFormValid = useMemo(
    () =>
      validateFeeSchedule({
        ratePercent: formData.ratePercent,
        startDate: formData.startDate,
        endDate: formData.endDate,
      }) === null,
    [formData.ratePercent, formData.startDate, formData.endDate],
  );

  const handleChange = useCallback(
    <K extends keyof FeeScheduleFormData>(field: K, value: FeeScheduleFormData[K]) => {
      setFormData((prev) => ({ ...prev, [field]: value }));
    },
    [],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const validationError = validateFeeSchedule({
        ratePercent: formData.ratePercent,
        startDate: formData.startDate,
        endDate: formData.endDate,
      });
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        const annualRatePercentMicros = decimalToMicro(formData.ratePercent);
        const endDate = formData.endDate || null;

        // FEE-060/061 — update leaves frequency and start_date untouched (immutable).
        const result = isExisting
          ? await accountDetailsGateway.updateFeeSchedule({
              account_id: accountId,
              asset_id: assetId,
              annual_rate_percent_micros: annualRatePercentMicros,
              end_date: endDate,
              active: formData.active,
            })
          : await accountDetailsGateway.createFeeSchedule({
              account_id: accountId,
              asset_id: assetId,
              annual_rate_percent_micros: annualRatePercentMicros,
              frequency: formData.frequency,
              start_date: formData.startDate,
              end_date: endDate,
            });

        if (result.status === "error") {
          logger.error("[useFeeSchedule] save failed", { error: result.error });
          setError(managementFeeErrorToI18n(result.error));
          return;
        }
        showSnackbar(t("fee_schedule.saved"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [accountId, assetId, formData, isExisting, t, showSnackbar, onSubmitSuccess],
  );

  const handleDelete = useCallback(async () => {
    setError(null);
    setIsSubmitting(true);
    try {
      const result = await accountDetailsGateway.deleteFeeSchedule(accountId, assetId);
      if (result.status === "error") {
        logger.error("[useFeeSchedule] delete failed", { error: result.error });
        setError(managementFeeErrorToI18n(result.error));
        return;
      }
      showSnackbar(t("fee_schedule.deleted"), "success");
      onSubmitSuccess?.();
    } finally {
      setIsSubmitting(false);
    }
  }, [accountId, assetId, t, showSnackbar, onSubmitSuccess]);

  return {
    formData,
    isExisting,
    isLoading,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
    handleDelete,
  };
}

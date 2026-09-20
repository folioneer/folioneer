import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThresholdDirection } from "@/bindings";
import { logger } from "@/lib/logger";
import { decimalToMicro, microToDecimal } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { holdingNoteErrorToI18n } from "../shared/presenter";
import type { HoldingNoteTarget } from "../shared/types";

/** HNO-011 — maximum note length after trimming (mirrors the backend check). */
export const NOTE_TEXT_MAX_LENGTH = 500;

interface UseHoldingNoteProps {
  accountId: string;
  target: HoldingNoteTarget;
  onSubmitSuccess?: () => void;
}

interface HoldingNoteFormData {
  text: string;
  /** HNO-042 — the "alert me" toggle revealing the direction + threshold fields. */
  alarmEnabled: boolean;
  direction: ThresholdDirection;
  /** Threshold as a decimal string in the asset currency (HNO-031). */
  price: string;
}

export function useHoldingNote({ accountId, target, onSubmitSuccess }: UseHoldingNoteProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  // HNO-020 — editing is a full replace: the form prefills from the stored note.
  const isEditMode = target.existing !== null;

  const [formData, setFormData] = useState<HoldingNoteFormData>(() => ({
    text: target.existing?.text ?? "",
    alarmEnabled: target.existing?.thresholdPrice != null,
    direction: target.existing?.thresholdDirection ?? "Below",
    price:
      target.existing?.thresholdPrice != null ? microToDecimal(target.existing.thresholdPrice) : "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const trimmedText = formData.text.trim();
  const thresholdPriceMicro = decimalToMicro(formData.price);

  // HNO-011 mirrored client-side: non-empty trimmed text within 500 chars; a
  // strictly positive threshold whenever the alarm toggle is on. The direction
  // select always carries a value, so ThresholdIncomplete cannot arise here.
  const isFormValid = useMemo(
    () =>
      trimmedText.length > 0 &&
      trimmedText.length <= NOTE_TEXT_MAX_LENGTH &&
      (!formData.alarmEnabled || thresholdPriceMicro > 0),
    [trimmedText, formData.alarmEnabled, thresholdPriceMicro],
  );

  const handleChange = useCallback(
    <K extends keyof HoldingNoteFormData>(field: K, value: HoldingNoteFormData[K]) => {
      setFormData((prev) => ({ ...prev, [field]: value }));
    },
    [],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!isFormValid) return;

      setError(null);
      setIsSubmitting(true);
      try {
        const result = await accountDetailsGateway.upsertHoldingNote({
          account_id: accountId,
          asset_id: target.assetId,
          text: trimmedText,
          threshold_price: formData.alarmEnabled ? thresholdPriceMicro : null,
          threshold_direction: formData.alarmEnabled ? formData.direction : null,
        });

        if (result.status === "error") {
          logger.error("[useHoldingNote] upsertHoldingNote failed", { error: result.error });
          setError(holdingNoteErrorToI18n(result.error));
          return;
        }

        showSnackbar(t("holding_note.saved"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      accountId,
      target.assetId,
      trimmedText,
      thresholdPriceMicro,
      formData.alarmEnabled,
      formData.direction,
      isFormValid,
      t,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  // HNO-021 — remove the stored note; the affordance only shows in edit mode.
  const handleDelete = useCallback(async () => {
    setError(null);
    setIsSubmitting(true);
    try {
      const result = await accountDetailsGateway.deleteHoldingNote({
        account_id: accountId,
        asset_id: target.assetId,
      });

      if (result.status === "error") {
        logger.error("[useHoldingNote] deleteHoldingNote failed", { error: result.error });
        setError(holdingNoteErrorToI18n(result.error));
        return;
      }

      showSnackbar(t("holding_note.deleted"), "success");
      onSubmitSuccess?.();
    } finally {
      setIsSubmitting(false);
    }
  }, [accountId, target.assetId, t, showSnackbar, onSubmitSuccess]);

  return {
    formData,
    isEditMode,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
    handleDelete,
  };
}

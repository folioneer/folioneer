import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { WriteOffReason } from "@/bindings";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { type WriteOffRefusal, writeOffErrorToI18n } from "../shared/presenter";
import type { WriteOffTarget } from "../shared/types";
import { validateAmount, validateDate } from "../shared/validateCashForm";

// Keyed by the generated union, so a new reason fails compilation until it is placed.
const REASONS: Record<WriteOffReason, true> = {
  NetworkFee: true,
  Dust: true,
  Lost: true,
  Other: true,
};

/** The four reasons, in the form's order (WOF-051): the record's declaration order. */
export const WRITE_OFF_REASONS = Object.keys(REASONS) as WriteOffReason[];

/** Edit-mode context (WOF-058): the write-off corrected; the asset is fixed. */
export interface WriteOffEditMode {
  transactionId: string;
  initialDate: string;
  /** The quantity as a field decimal. */
  initialQuantity: string;
  /** The quantity in micro-units, counted back into what is held (WOF-058). */
  initialQuantityMicro: number;
  initialReason: WriteOffReason;
  initialNote: string;
}

interface UseWriteOffProps {
  accountId: string;
  target: WriteOffTarget;
  onSubmitSuccess?: () => void;
  editMode?: WriteOffEditMode;
}

interface WriteOffFormData {
  date: string;
  reason: WriteOffReason;
  quantity: string;
  note: string;
}

/** The cost a quantity takes out at an average price, rounded down (WOF-021). */
function costOf(quantityMicro: number, averagePriceMicro: number): number {
  return Number((BigInt(quantityMicro) * BigInt(averagePriceMicro)) / 1_000_000n);
}

export function useWriteOff({ accountId, target, onSubmitSuccess, editMode }: UseWriteOffProps) {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<WriteOffFormData>(() => ({
    date: editMode?.initialDate ?? getLastOperationDate(accountId),
    reason: editMode?.initialReason ?? "NetworkFee",
    quantity: editMode?.initialQuantity ?? "",
    note: editMode?.initialNote ?? "",
  }));
  const [refusal, setRefusal] = useState<WriteOffRefusal | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // WOF-058 — when correcting, the write-off being edited counts back into what is held.
  const heldMicro = target.quantityMicro + (editMode?.initialQuantityMicro ?? 0);

  // WOF-053 — a figure that rounds to no micro-unit is no quantity.
  const quantityMicro = useMemo(() => {
    if (validateAmount(formData.quantity) !== null) return null;
    const micro = decimalToMicro(formData.quantity);
    return micro > 0 ? micro : null;
  }, [formData.quantity]);

  // WOF-053 — a quantity above the quantity held is refused under the field, without recording.
  const overHolding = quantityMicro !== null && quantityMicro > heldMicro;

  // WOF-052 — the form's estimate at the average price shown; the core takes the cost at the date.
  const estimatedCostMicro =
    quantityMicro !== null && !overHolding ? costOf(quantityMicro, target.averagePriceMicro) : null;

  const isFormValid =
    quantityMicro !== null && !overHolding && validateDate(formData.date) === null;

  const handleChange = useCallback((field: keyof WriteOffFormData, value: string) => {
    // WOF-055 — a refusal holds until the date, the quantity or the reason changes.
    if (field !== "note") setRefusal(null);
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (quantityMicro === null || overHolding) return;
      const dateErr = validateDate(formData.date);
      if (dateErr) {
        setRefusal({ message: dateErr, onQuantity: false });
        return;
      }

      setRefusal(null);
      setIsSubmitting(true);
      try {
        const note = formData.note || null;
        const result = editMode
          ? await accountDetailsGateway.correctWriteOff({
              account_id: accountId,
              transaction_id: editMode.transactionId,
              date: formData.date,
              quantity: quantityMicro,
              reason: formData.reason,
              note,
            })
          : await accountDetailsGateway.recordWriteOff({
              account_id: accountId,
              asset_id: target.assetId,
              date: formData.date,
              quantity: quantityMicro,
              reason: formData.reason,
              note,
            });

        if (result.status === "error") {
          logger.error("[useWriteOff] write-off refused", { error: result.error });
          setRefusal(
            writeOffErrorToI18n(result.error, target.assetReference, formData.date, i18n.language),
          );
          return;
        }
        if (!editMode) setLastOperationDate(accountId, formData.date);
        showSnackbar(t("write_off.recorded"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      accountId,
      target,
      formData,
      quantityMicro,
      overHolding,
      editMode,
      t,
      i18n.language,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  return {
    formData,
    refusal,
    isSubmitting,
    isFormValid,
    heldMicro,
    overHolding,
    estimatedCostMicro,
    handleChange,
    handleSubmit,
  };
}

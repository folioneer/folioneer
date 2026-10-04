import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TransactionDraft } from "@/bindings";
import { logger } from "@/lib/logger";
import { decimalToMicro, microToFormatted } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { dividendErrorToI18n } from "../shared/presenter";
import { useDraftProblemDisplay } from "../shared/useDraftProblemDisplay";
import { useTransactionDraftCheck } from "../shared/useTransactionDraftCheck";

/** The recorded dividend being corrected (DIV-040): its account and paying asset are fixed. */
export interface EditedDividend {
  transactionId: string;
  accountId: string;
  assetId: string;
  date: string;
  /** Decimal strings, as the fields show them. */
  amount: string;
  exchangeRate: string;
  note: string;
  /** The stored unit price, in micro-units: carried back unchanged. */
  unitPriceMicro: number;
}

interface UseEditDividendProps {
  dividend: EditedDividend;
  /** Called once the correction is recorded (the caller closes and refreshes). */
  onSubmitSuccess: () => void;
}

interface EditDividendFormData {
  date: string;
  amount: string;
  exchangeRate: string;
  note: string;
}

/**
 * DIV-040 — state of the dividend correction dialog. The core checks what is typed as
 * it changes and returns the total it would credit (TRX-062); the form shows the first
 * problem where it belongs (TRX-067) and decides nothing itself.
 */
export function useEditDividend({ dividend, onSubmitSuccess }: UseEditDividendProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<EditDividendFormData>(() => ({
    date: dividend.date,
    amount: dividend.amount,
    exchangeRate: dividend.exchangeRate,
    note: dividend.note,
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // The amount travels as the draft's quantity, as a dividend correction carries it. The
  // draft changes only when one of its values does: a caller's re-render is not a change.
  const { transactionId, accountId, assetId, unitPriceMicro } = dividend;
  const draft = useMemo<TransactionDraft>(
    () => ({
      kind: "Dividend",
      account_id: accountId,
      asset_id: assetId,
      date: formData.date,
      quantity: decimalToMicro(formData.amount),
      entered: {
        mode: "UnitPrice",
        unit_price: unitPriceMicro,
        exchange_rate: decimalToMicro(formData.exchangeRate),
        fees: 0,
      },
      correcting: transactionId,
    }),
    [
      transactionId,
      accountId,
      assetId,
      unitPriceMicro,
      formData.date,
      formData.amount,
      formData.exchangeRate,
    ],
  );
  const check = useTransactionDraftCheck(draft);
  const problemDisplay = useDraftProblemDisplay(check.problem, check.problemMessage);
  const touch = problemDisplay.touch;

  const handleChange = useCallback(
    (field: keyof EditDividendFormData, value: string) => {
      setFormData((previous) => ({ ...previous, [field]: value }));
      if (field === "date") touch("date");
      if (field === "amount") touch("quantity");
      if (field === "exchangeRate") touch("exchangeRate");
    },
    [touch],
  );

  const handleSubmit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (!check.preview) return;
      setError(null);
      setIsSubmitting(true);
      try {
        const result = await accountDetailsGateway.correctTransaction(
          dividend.transactionId,
          dividend.accountId,
          {
            date: formData.date,
            quantity: decimalToMicro(formData.amount),
            unit_price: dividend.unitPriceMicro,
            exchange_rate: decimalToMicro(formData.exchangeRate),
            fees: 0,
            total_amount: null,
            note: formData.note || null,
          },
        );
        if (result.status === "error") {
          logger.error("[useEditDividend] correctTransaction failed", { error: result.error });
          setError(dividendErrorToI18n(result.error));
          return;
        }
        showSnackbar(t("dividend.updated"), "success");
        onSubmitSuccess();
      } finally {
        setIsSubmitting(false);
      }
    },
    [check.preview, dividend, formData, onSubmitSuccess, showSnackbar, t],
  );

  return {
    formData,
    /** The total the core would credit to the account, formatted; null while unknown. */
    totalDisplay: check.preview ? microToFormatted(check.preview.total_amount) : null,
    fieldErrors: problemDisplay.fieldErrors,
    problemHint: problemDisplay.hint,
    error: error ?? problemDisplay.alert,
    isSubmitting,
    isFormValid: check.isClean,
    handleChange,
    handleSubmit,
  };
}

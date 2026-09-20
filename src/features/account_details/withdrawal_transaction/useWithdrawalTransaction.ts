import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { transactionMutationErrorToI18n } from "@/features/transactions/shared/presenter";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro, microToDecimal } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { validateAmount, validateDate } from "../shared/validateCashForm";

interface UseWithdrawalTransactionProps {
  accountId: string;
  /** When present, the modal edits this existing Withdrawal via correct_transaction (CSH-111). */
  editTransaction?: Transaction | null;
  onSubmitSuccess?: () => void;
}

interface WithdrawalFormData {
  date: string;
  amount: string;
  note: string;
}

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

export function useWithdrawalTransaction({
  accountId,
  editTransaction,
  onSubmitSuccess,
}: UseWithdrawalTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const isEdit = editTransaction != null;

  const [formData, setFormData] = useState<WithdrawalFormData>(() =>
    editTransaction
      ? {
          date: editTransaction.date,
          amount: microToDecimal(editTransaction.total_amount),
          note: editTransaction.note ?? "",
        }
      : { date: getLastOperationDate(accountId), amount: "", note: "" },
  );
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isFormValid = useMemo(
    () => validateAmount(formData.amount) === null && validateDate(formData.date) === null,
    [formData.amount, formData.date],
  );

  const handleChange = useCallback((field: keyof WithdrawalFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const amountErr = validateAmount(formData.amount);
      const dateErr = validateDate(formData.date);
      const validationError = amountErr ?? dateErr;
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        const amountMicros = decimalToMicro(formData.amount);
        // CSH-111 — edit reuses correct_transaction; create uses record_withdrawal.
        const result = editTransaction
          ? await accountDetailsGateway.correctTransaction(editTransaction.id, accountId, {
              date: formData.date,
              quantity: amountMicros,
              unit_price: editTransaction.unit_price,
              exchange_rate: editTransaction.exchange_rate,
              fees: editTransaction.fees,
              total_amount: null,
              note: formData.note || null,
            })
          : await accountDetailsGateway.recordWithdrawal({
              account_id: accountId,
              date: formData.date,
              amount_micros: amountMicros,
              note: formData.note || null,
            });
        if (result.status === "error") {
          logger.error("[useWithdrawalTransaction] submit failed", {
            error: result.error,
          });
          setError(transactionMutationErrorToI18n(result.error));
          return;
        }
        if (!isEdit) setLastOperationDate(accountId, formData.date);
        showSnackbar(t(isEdit ? "cash.withdrawal_updated" : "cash.withdrawal_recorded"), "success");
        onSubmitSuccess?.();
      } catch (e) {
        logger.error("Failed to save withdrawal", { error: e });
        setError(UNKNOWN_ERROR);
      } finally {
        setIsSubmitting(false);
      }
    },
    [accountId, editTransaction, isEdit, formData, t, showSnackbar, onSubmitSuccess],
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

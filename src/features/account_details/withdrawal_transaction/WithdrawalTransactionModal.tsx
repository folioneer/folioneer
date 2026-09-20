import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { TextField } from "@/ui/components/field/TextField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { useWithdrawalTransaction } from "./useWithdrawalTransaction";

interface WithdrawalTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  accountName: string;
  accountCurrency: string;
  /** When present, the modal edits this existing Withdrawal (CSH-111) instead of recording a new one. */
  editTransaction?: Transaction | null;
  onSubmitSuccess: () => void;
}

export function WithdrawalTransactionModal({
  isOpen,
  onClose,
  accountId,
  accountName,
  accountCurrency,
  editTransaction,
  onSubmitSuccess,
}: WithdrawalTransactionModalProps) {
  const { t } = useTranslation();
  const isEdit = editTransaction != null;

  useEffect(() => {
    logger.info("[WithdrawalTransactionModal] mounted");
  }, []);

  const { formData, error, isSubmitting, isFormValid, handleChange, handleSubmit } =
    useWithdrawalTransaction({ accountId, editTransaction, onSubmitSuccess });

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="withdrawal-transaction-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t(isEdit ? "action.save" : "cash.action_record_withdrawal")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, isEdit, t, onClose],
  );

  return (
    <FormModal
      id="withdrawal-transaction-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t(isEdit ? "cash.withdrawal_edit_modal_title" : "cash.withdrawal_modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form
        id="withdrawal-transaction-form"
        onSubmit={handleSubmit}
        className="flex flex-col gap-4"
      >
        <TextField
          id="withdrawal-trx-account"
          label={t("transaction.form_account_label")}
          type="text"
          value={accountName}
          readOnly
          aria-readonly="true"
        />

        <DateField
          id="withdrawal-trx-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        <CalcField
          id="withdrawal-trx-amount"
          label={`${t("cash.form_amount_label")} (${accountCurrency})`}
          value={formData.amount}
          onValueChange={(v) => handleChange("amount", v)}
          placeholder={t("cash.form_amount_placeholder")}
          required
        />

        <TextareaField
          id="withdrawal-trx-note"
          label={t("transaction.form_note_label")}
          rows={2}
          value={formData.note}
          onChange={(e) => handleChange("note", e.target.value)}
          placeholder={t("transaction.form_note_placeholder")}
        />

        {error && (
          <p role="alert" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </FormModal>
  );
}

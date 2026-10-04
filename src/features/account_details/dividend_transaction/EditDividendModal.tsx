import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { TextField } from "@/ui/components/field/TextField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { FormProblem } from "@/ui/components/modal/FormProblem";
import { messageText } from "@/ui/format/i18n";
import type { EditedDividend } from "./useEditDividend";
import { useEditDividend } from "./useEditDividend";

interface EditDividendModalProps {
  onClose: () => void;
  dividend: EditedDividend;
  /** Shown as the read-only line: neither can change on a dividend (DIV-040). */
  accountName: string;
  accountCurrency: string;
  assetName: string;
  assetCurrency: string;
  /**
   * DIV-029 — the dividend was recorded in the account's currency (what the account
   * received, at a rate of 1): its amount is in that currency and no rate applies.
   */
  amountInAccountCurrency: boolean;
  onSubmitSuccess: () => void;
}

/**
 * DIV-042 — corrects a recorded dividend: its date, its amount, its exchange rate when
 * the paying asset and the account differ in currency, and its note. A dividend has no
 * unit price, no fees and no market price to record.
 */
export function EditDividendModal({
  onClose,
  dividend,
  accountName,
  accountCurrency,
  assetName,
  assetCurrency,
  amountInAccountCurrency,
  onSubmitSuccess,
}: EditDividendModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[EditDividendModal] mounted");
  }, []);

  const {
    formData,
    totalDisplay,
    fieldErrors,
    problemHint,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
  } = useEditDividend({ dividend, onSubmitSuccess });

  // DIV-043 — a rate is asked only for an amount in another currency than the
  // account's; one recorded in the account's currency keeps its mode.
  const amountCurrency = amountInAccountCurrency ? accountCurrency : assetCurrency;
  const showExchangeRate = amountCurrency !== accountCurrency;

  const footer = (
    <div className="flex items-center justify-end gap-2">
      <FormProblem
        idPrefix="edit-dividend"
        error={messageText(t, error)}
        hint={messageText(t, problemHint)}
      />
      <Button
        id="edit-dividend-cancel"
        variant="secondary"
        onClick={onClose}
        disabled={isSubmitting}
      >
        {t("action.cancel")}
      </Button>
      <Button
        id="edit-dividend-save"
        type="submit"
        form="edit-dividend-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || !isFormValid}
      >
        {t("action.save")}
      </Button>
    </div>
  );

  return (
    <FormModal
      id="edit-dividend-modal"
      isOpen
      onClose={onClose}
      title={t("dividend.edit_modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="edit-dividend-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        <div className="flex flex-col text-sm">
          <span id="edit-dividend-asset" className="text-m3-on-surface">
            {assetName}
          </span>
          <span id="edit-dividend-locked-note" className="text-xs text-m3-on-surface-variant">
            {t("dividend.edit_locked_note", { account: accountName, currency: accountCurrency })}
          </span>
        </div>

        <DateField
          id="edit-dividend-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(event) => handleChange("date", event.target.value)}
          required
          error={messageText(t, fieldErrors.date)}
        />

        <div className={showExchangeRate ? "grid grid-cols-2 gap-4" : ""}>
          <CalcField
            id="edit-dividend-amount"
            label={`${t("dividend.form_amount_label")} (${amountCurrency})`}
            value={formData.amount}
            onValueChange={(value) => handleChange("amount", value)}
            placeholder={t("dividend.form_amount_placeholder")}
            required
            error={messageText(t, fieldErrors.quantity)}
          />
          {/* DIV-022 — the exchange rate only when the two currencies differ */}
          {showExchangeRate && (
            <CalcField
              id="edit-dividend-exchange-rate"
              label={`${t("transaction.form_exchange_rate_label")} (${assetCurrency} → ${accountCurrency})`}
              value={formData.exchangeRate}
              onValueChange={(value) => handleChange("exchangeRate", value)}
              placeholder={t("transaction.form_exchange_rate_placeholder")}
              required
              error={messageText(t, fieldErrors.exchangeRate)}
            />
          )}
        </div>

        {/* TRX-062 — the total is the core's, shown as it would be recorded */}
        <TextField
          id="edit-dividend-total"
          label={t("dividend.edit_total_label")}
          type="text"
          value={totalDisplay === null ? "—" : `${totalDisplay} ${accountCurrency}`}
          readOnly
          aria-readonly="true"
        />

        <TextareaField
          id="edit-dividend-note"
          label={t("transaction.form_note_label")}
          rows={2}
          value={formData.note}
          onChange={(event) => handleChange("note", event.target.value)}
          placeholder={t("transaction.form_note_placeholder")}
        />
      </form>
    </FormModal>
  );
}

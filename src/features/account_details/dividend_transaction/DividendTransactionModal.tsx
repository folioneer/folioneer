import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { ComboboxField } from "@/ui/components/field/ComboboxField";
import { DateField } from "@/ui/components/field/DateField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { DividendPayingAsset } from "./useDividendTransaction";
import { useDividendTransaction } from "./useDividendTransaction";

interface DividendTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  accountCurrency: string;
  /** Active non-cash holdings the dividend can be attributed to (DIV-011/020). */
  heldAssets: DividendPayingAsset[];
  onSubmitSuccess: () => void;
  /** Refresh-only callback for "Record & add another" — keeps the modal open (DIV-010). */
  onRecorded: () => void;
}

export function DividendTransactionModal({
  isOpen,
  onClose,
  accountId,
  accountCurrency,
  heldAssets,
  onSubmitSuccess,
  onRecorded,
}: DividendTransactionModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[DividendTransactionModal] mounted");
  }, []);

  const {
    formData,
    error,
    isSubmitting,
    isFormValid,
    showExchangeRate,
    showCurrencyModeSwitch,
    amountInAccountCurrency,
    setAmountInAccountCurrency,
    amountCurrency,
    handleChange,
    handleSubmit,
    handleAddAnother,
  } = useDividendTransaction({
    accountId,
    accountCurrency,
    heldAssets,
    onSubmitSuccess,
    onRecorded,
  });

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          id="dividend-trx-add-another"
          type="button"
          variant="secondary"
          onClick={handleAddAnother}
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("dividend.action_record_and_add_another")}
        </Button>
        <Button
          id="dividend-trx-record"
          type="submit"
          form="dividend-transaction-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("dividend.action_record")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose, handleAddAnother],
  );

  return (
    <FormModal
      id="dividend-transaction-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("dividend.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="dividend-transaction-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* DIV-020 — paying asset chosen inside the modal via fuzzy-search combobox */}
        <ComboboxField
          id="dividend-trx-asset"
          label={t("dividend.form_asset_label")}
          items={heldAssets}
          displayKey="assetName"
          idKey="assetId"
          value={formData.assetId}
          onChange={(id) => handleChange("assetId", id)}
          searchKeys={["assetName"]}
          placeholder={t("dividend.form_select_asset")}
        />

        <DateField
          id="dividend-trx-date"
          data-testid="dividend-trx-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* DIV-028 — entry-mode switch, only when the currencies differ */}
        {showCurrencyModeSwitch && (
          <label className="flex items-center gap-3 cursor-pointer group">
            <input
              id="dividend-trx-account-currency-mode"
              data-testid="dividend-trx-account-currency-mode"
              type="checkbox"
              checked={amountInAccountCurrency}
              onChange={(e) => setAmountInAccountCurrency(e.target.checked)}
              className="accent-m3-primary w-4 h-4"
            />
            <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
              {t("dividend.form_account_currency_mode", { currency: accountCurrency })}
            </span>
          </label>
        )}

        {/* DIV-021/028 — amount in the selected entry currency */}
        <CalcField
          id="dividend-trx-amount"
          data-testid="dividend-trx-amount"
          label={`${t("dividend.form_amount_label")} (${amountCurrency})`}
          value={formData.amount}
          onValueChange={(v) => handleChange("amount", v)}
          placeholder={t("dividend.form_amount_placeholder")}
          required
        />

        {/* DIV-022 — exchange rate only when asset currency differs from account currency */}
        {showExchangeRate && (
          <CalcField
            id="dividend-trx-exchange-rate"
            data-testid="dividend-trx-exchange-rate"
            label={t("transaction.form_exchange_rate_label")}
            value={formData.exchangeRate}
            onValueChange={(v) => handleChange("exchangeRate", v)}
            placeholder={t("transaction.form_exchange_rate_placeholder")}
          />
        )}

        <TextareaField
          id="dividend-trx-note"
          data-testid="dividend-trx-note"
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

import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { EntryModeToggle } from "@/features/transactions/shared/EntryModeToggle";
import { RecordPriceCheckbox } from "@/features/transactions/shared/RecordPriceCheckbox";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { TextField } from "@/ui/components/field/TextField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { FormModal } from "@/ui/components/modal/FormModal";
import { useBuyTransaction } from "./useBuyTransaction";

interface BuyTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  accountName: string;
  assetId: string;
  assetName: string;
  assetCurrency: string;
  /** When true, asset currency differs from account currency — show exchange rate field (TRX-041). */
  showExchangeRate?: boolean;
  /** Called after a successful purchase submission. Caller must refresh data. */
  onSubmitSuccess: () => void;
}

export function BuyTransactionModal({
  isOpen,
  onClose,
  accountId,
  accountName,
  assetId,
  assetName,
  assetCurrency,
  showExchangeRate = false,
  onSubmitSuccess,
}: BuyTransactionModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[BuyTransactionModal] mounted");
  }, []);

  const {
    formData,
    totalAmountDisplay,
    entryMode,
    setEntryMode,
    totalAmountInput,
    handleTotalAmountChange,
    totalBelowFeesError,
    unitPriceDisplay,
    averageCostAsOfDate,
    error,
    isSubmitting,
    isFormValid,
    showArchivedConfirm,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  } = useBuyTransaction({ accountId, assetId, onSubmitSuccess });

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button
          variant="secondary"
          onClick={onClose}
          disabled={isSubmitting || showArchivedConfirm}
        >
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="buy-transaction-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || showArchivedConfirm || !isFormValid}
        >
          {t("transaction.action_buy")}
        </Button>
      </div>
    ),
    [isSubmitting, showArchivedConfirm, isFormValid, t, onClose],
  );

  return (
    <>
      <FormModal
        id="buy-transaction-modal"
        isOpen={isOpen}
        onClose={onClose}
        title={t("transaction.buy_modal_title")}
        footer={footer}
        maxWidth="max-w-2xl"
      >
        <form id="buy-transaction-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
          {/* Account + Asset (read-only, TRX-011) */}
          <div className="grid grid-cols-2 gap-4">
            <TextField
              id="buy-trx-account"
              label={t("transaction.form_account_label")}
              type="text"
              value={accountName}
              readOnly
              aria-readonly="true"
            />
            <TextField
              id="buy-trx-asset"
              label={t("transaction.form_asset_label")}
              type="text"
              value={assetName}
              readOnly
              aria-readonly="true"
            />
          </div>

          {/* Date */}
          <DateField
            id="buy-trx-date"
            label={t("transaction.form_date_label")}
            value={formData.date}
            onChange={(e) => handleChange("date", e.target.value)}
            required
          />

          {/* Quantity */}
          <CalcField
            id="buy-trx-quantity"
            label={t("transaction.form_quantity_label")}
            value={formData.quantity}
            onValueChange={(v) => handleChange("quantity", v)}
            placeholder={t("transaction.form_quantity_placeholder")}
            required
          />

          {/* TRX-060 — entry mode: type the unit price or the broker's all-in total */}
          <EntryModeToggle idPrefix="buy-trx" value={entryMode} onChange={setEntryMode} />

          {/* Unit Price + average-cost insight (TDI-020) */}
          <div className="flex flex-col gap-1">
            {entryMode === "price" ? (
              <CalcField
                id="buy-trx-unit-price"
                label={`${t("transaction.form_unit_price_label")} (${assetCurrency})`}
                value={formData.unitPrice}
                onValueChange={(v) => handleChange("unitPrice", v)}
                placeholder={t("transaction.form_unit_price_placeholder")}
                required
              />
            ) : (
              // TRX-060 — derived from the typed total; the backend recomputes it authoritatively
              <TextField
                id="buy-trx-unit-price"
                label={`${t("transaction.form_unit_price_label")} (${assetCurrency})`}
                type="text"
                value={unitPriceDisplay}
                readOnly
                aria-readonly="true"
              />
            )}
            {averageCostAsOfDate !== null && (
              <span id="buy-trx-avg-cost" className="text-xs text-m3-on-surface-variant">
                {t("transaction.form_avg_cost_hint", { value: averageCostAsOfDate })}
              </span>
            )}
          </div>

          {/* Exchange Rate (TRX-041) */}
          {showExchangeRate && (
            <CalcField
              id="buy-trx-exchange-rate"
              label={t("transaction.form_exchange_rate_label")}
              value={formData.exchangeRate}
              onValueChange={(v) => handleChange("exchangeRate", v)}
              placeholder={t("transaction.form_exchange_rate_placeholder")}
            />
          )}

          {/* Fees + Total */}
          <div className="grid grid-cols-2 gap-4">
            <CalcField
              id="buy-trx-fees"
              label={t("transaction.form_fees_label")}
              value={formData.fees}
              onValueChange={(v) => handleChange("fees", v)}
              placeholder={t("transaction.form_fees_placeholder")}
            />
            {entryMode === "price" ? (
              <TextField
                id="buy-trx-total"
                label={t("transaction.form_total_amount_label")}
                type="text"
                value={totalAmountDisplay}
                readOnly
                aria-readonly="true"
              />
            ) : (
              <CalcField
                id="buy-trx-total"
                label={t("transaction.form_total_amount_label")}
                value={totalAmountInput}
                onValueChange={handleTotalAmountChange}
                placeholder={t("transaction.form_total_amount_placeholder")}
                error={
                  totalBelowFeesError
                    ? t(totalBelowFeesError.key, totalBelowFeesError.vars)
                    : undefined
                }
                required
              />
            )}
          </div>

          {/* Note */}
          <TextareaField
            id="buy-trx-note"
            label={t("transaction.form_note_label")}
            rows={2}
            value={formData.note}
            onChange={(e) => handleChange("note", e.target.value)}
            placeholder={t("transaction.form_note_placeholder")}
          />

          {/* Auto-record price (MKT-051) */}
          <RecordPriceCheckbox
            checked={recordPrice}
            onChange={setRecordPrice}
            date={formData.date}
          />

          {/* Inline error */}
          {error && (
            <p role="alert" className="text-sm text-m3-error">
              {t(error.key, error.vars)}
            </p>
          )}
        </form>
      </FormModal>

      {/* TRX-029 — archived asset confirmation */}
      <ConfirmationDialog
        isOpen={showArchivedConfirm}
        onCancel={handleCancelArchived}
        onConfirm={handleConfirmArchived}
        title={t("transaction.archived_asset_confirm_title")}
        message={t("transaction.archived_asset_confirm_message")}
        confirmLabel={t("action.confirm")}
        cancelLabel={t("action.cancel")}
      />
    </>
  );
}

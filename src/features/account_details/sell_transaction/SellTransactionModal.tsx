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
import { FormModal } from "@/ui/components/modal/FormModal";
import { useSellTransaction } from "./useSellTransaction";

interface SellTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  accountName: string;
  assetId: string;
  assetName: string;
  assetCurrency: string;
  /** Holding quantity in micro-units — used for max hint and oversell guard (SEL-022). */
  holdingQuantityMicro: number;
  /** When true, asset currency differs from account currency — show exchange rate field (SEL-036). */
  showExchangeRate?: boolean;
  /** Called after a successful sell submission (SEL-045). Required — caller must refresh data. */
  onSubmitSuccess: () => void;
}

export function SellTransactionModal({
  isOpen,
  onClose,
  accountId,
  accountName,
  assetId,
  assetName,
  assetCurrency,
  holdingQuantityMicro,
  showExchangeRate = false,
  onSubmitSuccess,
}: SellTransactionModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[SellTransactionModal] mounted");
  }, []);

  const {
    formData,
    totalAmountDisplay,
    maxQuantityDisplay,
    entryMode,
    setEntryMode,
    totalAmountInput,
    handleTotalAmountChange,
    unitPriceDisplay,
    averageCostAsOfDate,
    potentialPnl,
    error,
    isSubmitting,
    isFormValid,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
  } = useSellTransaction({
    accountId,
    assetId,
    holdingQuantityMicro,
    onSubmitSuccess,
  });

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="sell-transaction-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("transaction.action_sell")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="sell-transaction-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("transaction.sell_modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="sell-transaction-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* Account + Asset (read-only, SEL-011) */}
        <div className="grid grid-cols-2 gap-4">
          <TextField
            id="sell-trx-account"
            label={t("transaction.form_account_label")}
            type="text"
            value={accountName}
            readOnly
            aria-readonly="true"
          />
          <TextField
            id="sell-trx-asset"
            label={t("transaction.form_asset_label")}
            type="text"
            value={assetName}
            readOnly
            aria-readonly="true"
          />
        </div>

        {/* Date */}
        <DateField
          id="sell-trx-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* Quantity with max hint (SEL-022) */}
        <div className="flex flex-col gap-1">
          <CalcField
            id="sell-trx-quantity"
            label={t("transaction.form_quantity_label")}
            value={formData.quantity}
            onValueChange={(v) => handleChange("quantity", v)}
            placeholder={t("transaction.form_quantity_placeholder")}
            required
          />
          <span className="text-xs text-m3-on-surface-variant">
            {t("transaction.form_max_quantity_hint", {
              max: maxQuantityDisplay,
            })}
          </span>
        </div>

        {/* SEL-050 — entry mode: type the unit price or the broker's all-in net proceeds */}
        <EntryModeToggle idPrefix="sell-trx" value={entryMode} onChange={setEntryMode} />

        {/* Unit Price + average-cost insight (TDI-020) */}
        <div className="flex flex-col gap-1">
          {entryMode === "price" ? (
            <CalcField
              id="sell-trx-unit-price"
              label={`${t("transaction.form_unit_price_label")} (${assetCurrency})`}
              value={formData.unitPrice}
              onValueChange={(v) => handleChange("unitPrice", v)}
              placeholder={t("transaction.form_unit_price_placeholder")}
              required
            />
          ) : (
            // SEL-050 — derived from the typed proceeds; the backend recomputes it authoritatively
            <TextField
              id="sell-trx-unit-price"
              label={`${t("transaction.form_unit_price_label")} (${assetCurrency})`}
              type="text"
              value={unitPriceDisplay}
              readOnly
              aria-readonly="true"
            />
          )}
          {averageCostAsOfDate !== null && (
            <span id="sell-trx-avg-cost" className="text-xs text-m3-on-surface-variant">
              {t("transaction.form_avg_cost_hint", { value: averageCostAsOfDate })}
            </span>
          )}
        </div>

        {/* Exchange Rate (SEL-036) */}
        {showExchangeRate && (
          <CalcField
            id="sell-trx-exchange-rate"
            label={t("transaction.form_exchange_rate_label")}
            value={formData.exchangeRate}
            onValueChange={(v) => handleChange("exchangeRate", v)}
            placeholder={t("transaction.form_exchange_rate_placeholder")}
          />
        )}

        {/* Fees + Total Proceeds */}
        <div className="grid grid-cols-2 gap-4">
          <CalcField
            id="sell-trx-fees"
            label={t("transaction.form_fees_label")}
            value={formData.fees}
            onValueChange={(v) => handleChange("fees", v)}
            placeholder={t("transaction.form_fees_placeholder")}
          />
          {entryMode === "price" ? (
            <TextField
              id="sell-trx-total"
              label={t("transaction.form_total_amount_label")}
              type="text"
              value={totalAmountDisplay}
              readOnly
              aria-readonly="true"
            />
          ) : (
            <CalcField
              id="sell-trx-total"
              label={t("transaction.form_total_amount_label")}
              value={totalAmountInput}
              onValueChange={handleTotalAmountChange}
              placeholder={t("transaction.form_total_amount_placeholder")}
              required
            />
          )}
        </div>

        {/* Potential realized P&L of the typed sell (TDI-030/032) */}
        {potentialPnl !== null && (
          <span
            id="sell-trx-potential-pnl"
            className={`text-xs ${potentialPnl.raw < 0 ? "text-m3-loss" : "text-m3-gain"}`}
          >
            {t("transaction.form_potential_pnl_hint", { value: potentialPnl.formatted })}
          </span>
        )}

        {/* Note */}
        <TextareaField
          id="sell-trx-note"
          label={t("transaction.form_note_label")}
          rows={2}
          value={formData.note}
          onChange={(e) => handleChange("note", e.target.value)}
          placeholder={t("transaction.form_note_placeholder")}
        />

        {/* Auto-record price (MKT-051) */}
        <RecordPriceCheckbox checked={recordPrice} onChange={setRecordPrice} date={formData.date} />

        {/* Inline error */}
        {error && (
          <p role="alert" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </FormModal>
  );
}

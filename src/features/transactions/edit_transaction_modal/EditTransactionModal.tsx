import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { ComboboxField } from "@/ui/components/field/ComboboxField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { TextField } from "@/ui/components/field/TextField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { FormModal } from "@/ui/components/modal/FormModal";
import { EntryModeToggle } from "../shared/EntryModeToggle";
import { RecordPriceCheckbox } from "../shared/RecordPriceCheckbox";
import { useEditTransactionModal } from "./useEditTransactionModal";

interface EditTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called only after a confirmed successful save (not on cancel). */
  onSuccess?: () => void;
  transaction: Transaction;
  /** Called when the user wants to create a new asset not yet in the catalog. */
  onCreateNewAsset?: (query: string) => void;
}

export function EditTransactionModal({
  isOpen,
  onClose,
  onSuccess,
  transaction,
  onCreateNewAsset,
}: EditTransactionModalProps) {
  const { t } = useTranslation();
  useEffect(() => {
    logger.info("[EditTransactionModal] mounted");
  }, []);

  // CSH-018 — Cash Assets cannot be the target of a manual edit (Deposit/Withdrawal flow only).
  const assets = useAppStore((state) => state.assets).filter((a) => a.class !== "Cash");
  const accounts = useAppStore((state) => state.accounts);

  const {
    formData,
    totalAmountDisplay,
    error,
    isSubmitting,
    isFormValid,
    showArchivedConfirm,
    recordPrice,
    setRecordPrice,
    isTotalEntryEligible,
    isTotalMode,
    entryMode,
    handleEntryModeChange,
    totalAmountInput,
    handleTotalAmountChange,
    totalBelowFeesError,
    unitPriceDisplay,
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  } = useEditTransactionModal({
    transaction,
    onSubmitSuccess: onSuccess ?? onClose,
  });

  const isOpeningBalance = transaction.transaction_type === "OpeningBalance";
  const selectedAsset = assets.find((a) => a.id === formData.assetId);
  const selectedAccount = accounts.find((a) => a.id === formData.accountId);
  const showExchangeRate =
    !isOpeningBalance &&
    (selectedAsset && selectedAccount ? selectedAsset.currency !== selectedAccount.currency : true);

  const accountOptions = accounts.map((a) => ({ label: a.name, value: a.id }));

  const footer = (
    <div className="flex items-center justify-end gap-2">
      <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="edit-transaction-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || showArchivedConfirm || !isFormValid}
      >
        {t("action.save")}
      </Button>
    </div>
  );

  return (
    <>
      <FormModal
        id="edit-transaction-modal"
        isOpen={isOpen}
        onClose={onClose}
        title={t("transaction.edit_modal_title")}
        footer={footer}
        maxWidth="max-w-2xl"
      >
        <form id="edit-transaction-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
          {/* Account */}
          <SelectField
            id="edit-trx-account"
            label={t("transaction.form_account_label")}
            value={formData.accountId}
            onChange={(e) => handleChange("accountId", e.target.value)}
            options={[{ label: `— ${t("action.select")} —`, value: "" }, ...accountOptions]}
            required
          />

          {/* Asset */}
          <ComboboxField
            id="edit-trx-asset"
            label={`${t("transaction.form_asset_label")} *`}
            items={assets}
            displayKey="name"
            idKey="id"
            value={formData.assetId}
            onChange={(id) => handleChange("assetId", id)}
            searchKeys={["name", "reference"]}
            placeholder={t("transaction.form_asset_placeholder")}
            onCreateNew={onCreateNewAsset}
            createLabel={onCreateNewAsset ? t("asset.create_new") : undefined}
          />

          {/* Date — TRX-046: cap at today for OpeningBalance */}
          <DateField
            id="edit-trx-date"
            label={t("transaction.form_date_label")}
            value={formData.date}
            onChange={(e) => handleChange("date", e.target.value)}
            max={isOpeningBalance ? new Date().toISOString().slice(0, 10) : undefined}
            required
          />

          {/* Quantity + Unit Price (or Total Cost for OpeningBalance — TRX-051) */}
          <div className="grid grid-cols-2 gap-4">
            <CalcField
              id="edit-trx-quantity"
              label={t("transaction.form_quantity_label")}
              value={formData.quantity}
              onValueChange={(v) => handleChange("quantity", v)}
              placeholder={t("transaction.form_quantity_placeholder")}
              required
            />
            {isTotalMode ? (
              // TRX-061 / SEL-051 — derived from the typed total; the backend recomputes it
              <TextField
                id="edit-trx-unit-price"
                label={`${t("transaction.form_unit_price_label")}${selectedAsset ? ` (${selectedAsset.currency})` : ""}`}
                type="text"
                value={unitPriceDisplay}
                readOnly
                aria-readonly="true"
              />
            ) : (
              <CalcField
                id={isOpeningBalance ? "edit-trx-total-cost" : "edit-trx-unit-price"}
                label={
                  isOpeningBalance
                    ? t("open_balance.form_total_cost_label")
                    : `${t("transaction.form_unit_price_label")}${selectedAsset ? ` (${selectedAsset.currency})` : ""}`
                }
                value={formData.unitPrice}
                onValueChange={(v) => handleChange("unitPrice", v)}
                placeholder={t("transaction.form_unit_price_placeholder")}
                required
              />
            )}
          </div>

          {/* TRX-061 / SEL-051 — entry mode: type the unit price or the broker's all-in total */}
          {isTotalEntryEligible && (
            <EntryModeToggle
              idPrefix="edit-trx"
              value={entryMode}
              onChange={handleEntryModeChange}
            />
          )}

          {/* Exchange Rate — hidden for OpeningBalance (TRX-051) */}
          {showExchangeRate && (
            <CalcField
              id="edit-trx-exchange-rate"
              label={t("transaction.form_exchange_rate_label")}
              value={formData.exchangeRate}
              onValueChange={(v) => handleChange("exchangeRate", v)}
              placeholder={t("transaction.form_exchange_rate_placeholder")}
            />
          )}

          {/* Fees + Total Amount (fees hidden for OpeningBalance — TRX-051) */}
          {!isOpeningBalance && (
            <div className="grid grid-cols-2 gap-4">
              <CalcField
                id="edit-trx-fees"
                label={t("transaction.form_fees_label")}
                value={formData.fees}
                onValueChange={(v) => handleChange("fees", v)}
                placeholder={t("transaction.form_fees_placeholder")}
              />
              {isTotalMode ? (
                // TRX-061 / SEL-051 — the typed all-in total is ground truth
                <CalcField
                  id="edit-trx-total"
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
              ) : (
                <TextField
                  id="edit-trx-total"
                  label={t("transaction.form_total_amount_label")}
                  type="text"
                  value={totalAmountDisplay}
                  readOnly
                  aria-readonly="true"
                />
              )}
            </div>
          )}

          {/* Note — not shown for OpeningBalance (TRX-043) */}
          {!isOpeningBalance && (
            <TextareaField
              id="edit-trx-note"
              label={t("transaction.form_note_label")}
              rows={2}
              value={formData.note}
              onChange={(e) => handleChange("note", e.target.value)}
              placeholder={t("transaction.form_note_placeholder")}
            />
          )}

          {/* Auto-record price (MKT-051) — not applicable for OpeningBalance */}
          {!isOpeningBalance && (
            <RecordPriceCheckbox
              checked={recordPrice}
              onChange={setRecordPrice}
              date={formData.date}
            />
          )}

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

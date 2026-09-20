import { useEffect } from "react";
import { useTranslation } from "react-i18next";
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
import { RecordPriceCheckbox } from "../shared/RecordPriceCheckbox";
import { useAddTransaction } from "./useAddTransaction";

interface AddTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Pre-fill the asset (TRX-011). */
  prefillAssetId?: string;
  /** Pre-fill the account (TRX-011). */
  prefillAccountId?: string;
  /** Called when the user wants to create a new asset not yet in the catalog. */
  onCreateNewAsset?: (query: string) => void;
}

export function AddTransactionModal({
  isOpen,
  onClose,
  prefillAssetId,
  prefillAccountId,
  onCreateNewAsset,
}: AddTransactionModalProps) {
  const { t } = useTranslation();
  useEffect(() => {
    logger.info("[AddTransactionModal] mounted");
  }, []);

  // CSH-018 — Cash Assets are managed via Deposit/Withdrawal flows; never selectable here.
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
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  } = useAddTransaction({
    prefillAssetId,
    prefillAccountId,
    onSubmitSuccess: onClose,
  });

  const selectedAsset = assets.find((a) => a.id === formData.assetId);
  const selectedAccount = accounts.find((a) => a.id === formData.accountId);
  const showExchangeRate =
    selectedAsset && selectedAccount ? selectedAsset.currency !== selectedAccount.currency : true;

  const accountOptions = accounts.map((a) => ({ label: a.name, value: a.id }));

  const footer = (
    <div className="flex items-center justify-end gap-2">
      <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="add-transaction-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || showArchivedConfirm || !isFormValid}
      >
        {t("action.add")}
      </Button>
    </div>
  );

  return (
    <>
      <FormModal
        id="add-transaction-modal"
        isOpen={isOpen}
        onClose={onClose}
        title={t("transaction.add_modal_title")}
        footer={footer}
        maxWidth="max-w-2xl"
      >
        <form id="add-transaction-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
          {/* Account */}
          <SelectField
            id="trx-account"
            label={t("transaction.form_account_label")}
            value={formData.accountId}
            onChange={(e) => handleChange("accountId", e.target.value)}
            options={[{ label: `— ${t("action.select")} —`, value: "" }, ...accountOptions]}
            required
          />

          {/* Asset */}
          <ComboboxField
            id="trx-asset"
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

          {/* Date */}
          <DateField
            id="trx-date"
            label={t("transaction.form_date_label")}
            value={formData.date}
            onChange={(e) => handleChange("date", e.target.value)}
            required
          />

          {/* Quantity + Unit Price side by side */}
          <div className="grid grid-cols-2 gap-4">
            <CalcField
              id="trx-quantity"
              label={t("transaction.form_quantity_label")}
              value={formData.quantity}
              onValueChange={(v) => handleChange("quantity", v)}
              placeholder={t("transaction.form_quantity_placeholder")}
              required
            />
            <CalcField
              id="trx-unit-price"
              label={`${t("transaction.form_unit_price_label")}${selectedAsset ? ` (${selectedAsset.currency})` : ""}`}
              value={formData.unitPrice}
              onValueChange={(v) => handleChange("unitPrice", v)}
              placeholder={t("transaction.form_unit_price_placeholder")}
              required
            />
          </div>

          {/* Exchange Rate (hidden when asset currency = account currency) */}
          {showExchangeRate && (
            <CalcField
              id="trx-exchange-rate"
              label={t("transaction.form_exchange_rate_label")}
              value={formData.exchangeRate}
              onValueChange={(v) => handleChange("exchangeRate", v)}
              placeholder={t("transaction.form_exchange_rate_placeholder")}
            />
          )}

          {/* Fees + Total Amount side by side */}
          <div className="grid grid-cols-2 gap-4">
            <CalcField
              id="trx-fees"
              label={t("transaction.form_fees_label")}
              value={formData.fees}
              onValueChange={(v) => handleChange("fees", v)}
              placeholder={t("transaction.form_fees_placeholder")}
            />
            <TextField
              id="trx-total"
              label={t("transaction.form_total_amount_label")}
              type="text"
              value={totalAmountDisplay}
              readOnly
              aria-readonly="true"
            />
          </div>

          {/* Note */}
          <TextareaField
            id="trx-note"
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

import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
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
import { useAddTransaction } from "../add_transaction/useAddTransaction";

export function AddTransactionPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { prefillAssetId, prefillAccountId } = useSearch({
    from: "/transactions/new",
  });

  useEffect(() => {
    logger.info("[AddTransactionPage] mounted");
  }, []);

  const assets = useAppStore((s) => s.assets);
  const accounts = useAppStore((s) => s.accounts);

  const handleBack = useCallback(() => {
    if (prefillAccountId) {
      navigate({
        to: "/accounts/$accountId",
        params: { accountId: prefillAccountId },
      });
    } else {
      navigate({
        to: "/assets",
        search: { createNew: undefined, returnPath: undefined },
      });
    }
  }, [navigate, prefillAccountId]);

  const {
    formData,
    totalAmountDisplay,
    error,
    isSubmitting,
    isFormValid,
    showArchivedConfirm,
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  } = useAddTransaction({
    prefillAssetId,
    prefillAccountId,
    onSubmitSuccess: handleBack,
  });

  const selectedAsset = assets.find((a) => a.id === formData.assetId);
  const selectedAccount = accounts.find((a) => a.id === formData.accountId);
  const showExchangeRate = !!selectedAsset && !!selectedAccount && selectedAsset.currency !== "EUR";

  const accountOptions = accounts.map((a) => ({ label: a.name, value: a.id }));

  const handleCreateNewAsset = useCallback(
    (query: string) => {
      const returnPath = prefillAccountId
        ? `/transactions/new?prefillAccountId=${prefillAccountId}`
        : "/transactions/new";
      navigate({
        to: "/assets",
        search: { createNew: query, returnPath },
      });
    },
    [navigate, prefillAccountId],
  );

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden py-2 px-2">
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        {/* Form */}
        <div className="flex-1 overflow-auto px-6 py-6">
          <form
            id="add-transaction-form"
            onSubmit={handleSubmit}
            className="flex flex-col gap-4 max-w-2xl"
          >
            {/* Account — locked if prefillAccountId provided */}
            {prefillAccountId ? (
              <TextField
                id="trx-account"
                label={t("transaction.form_account_label")}
                value={selectedAccount?.name ?? prefillAccountId}
                readOnly
                aria-readonly="true"
              />
            ) : (
              <SelectField
                id="trx-account"
                label={t("transaction.form_account_label")}
                value={formData.accountId}
                onChange={(e) => handleChange("accountId", e.target.value)}
                options={[{ label: `— ${t("action.select")} —`, value: "" }, ...accountOptions]}
                required
              />
            )}

            {/* Asset — locked if prefillAssetId provided */}
            {prefillAssetId ? (
              <TextField
                id="trx-asset"
                label={t("transaction.form_asset_label")}
                value={selectedAsset?.name ?? prefillAssetId}
                readOnly
                aria-readonly="true"
              />
            ) : (
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
                onCreateNew={handleCreateNewAsset}
                createLabel={t("asset.create_new")}
              />
            )}

            {/* Date */}
            <DateField
              id="trx-date"
              label={t("transaction.form_date_label")}
              value={formData.date}
              onChange={(e) => handleChange("date", e.target.value)}
              required
            />

            {/* Quantity + Unit Price */}
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

            {/* Exchange Rate */}
            {showExchangeRate && (
              <CalcField
                id="trx-exchange-rate"
                label={t("transaction.form_exchange_rate_label")}
                value={formData.exchangeRate}
                onValueChange={(v) => handleChange("exchangeRate", v)}
                placeholder={t("transaction.form_exchange_rate_placeholder")}
              />
            )}

            {/* Fees + Total */}
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

            {error && (
              <p role="alert" className="text-sm text-m3-error">
                {t(error.key, error.vars)}
              </p>
            )}
          </form>
        </div>

        {/* Footer actions */}
        <div className="px-6 py-4 bg-m3-surface-container-high flex justify-end gap-2">
          <Button variant="secondary" onClick={handleBack} disabled={isSubmitting}>
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
      </div>

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
    </div>
  );
}

import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { microToFormattedQuantity } from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { ManagementFeeHolding } from "./useManagementFee";
import { useManagementFee } from "./useManagementFee";

interface ManagementFeeModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** Active non-cash holdings the fee can be charged against (FEE-011/012). */
  heldAssets: ManagementFeeHolding[];
  onSubmitSuccess: () => void;
}

export function ManagementFeeModal({
  isOpen,
  onClose,
  accountId,
  heldAssets,
  onSubmitSuccess,
}: ManagementFeeModalProps) {
  const { t, i18n } = useTranslation();

  useEffect(() => {
    logger.info("[ManagementFeeModal] mounted");
  }, []);

  const {
    formData,
    error,
    preview,
    previewError,
    isSubmitting,
    isFormValid,
    handleChange,
    handleModeChange,
    handleSubmit,
  } = useManagementFee({ accountId, onSubmitSuccess });

  const assetOptions = useMemo(
    () => [
      { label: t("management_fee.form_select_asset"), value: "" },
      ...heldAssets.map((a) => ({ label: a.assetName, value: a.assetId })),
    ],
    [heldAssets, t],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button
          id="management-fee-cancel"
          variant="secondary"
          onClick={onClose}
          disabled={isSubmitting}
        >
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="management-fee-form"
          id="management-fee-submit"
          data-testid="management-fee-submit"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("management_fee.action_record")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="management-fee-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("management_fee.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="management-fee-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* FEE-011 — charged asset chosen inside the modal */}
        <SelectField
          id="management-fee-asset-select"
          data-testid="management-fee-asset-select"
          label={t("management_fee.form_asset_label")}
          value={formData.assetId}
          onChange={(e) => handleChange("assetId", e.target.value)}
          options={assetOptions}
          required
        />

        <DateField
          id="management-fee-date"
          data-testid="management-fee-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        <div className="flex flex-col gap-2">
          {/* FEE-020 — the fee is a percentage of the holding or the quantity it should end at */}
          <div
            role="radiogroup"
            aria-label={t("management_fee.mode_label")}
            className="flex w-fit gap-1 rounded-2xl bg-m3-surface-variant p-1"
          >
            <Button
              id="management-fee-mode-percent"
              size="sm"
              role="radio"
              aria-checked={formData.mode === "percent"}
              variant={formData.mode === "percent" ? "primary" : "ghost"}
              onClick={() => handleModeChange("percent")}
            >
              {t("management_fee.mode_percent")}
            </Button>
            <Button
              id="management-fee-mode-quantity"
              size="sm"
              role="radio"
              aria-checked={formData.mode === "quantity"}
              variant={formData.mode === "quantity" ? "primary" : "ghost"}
              onClick={() => handleModeChange("quantity")}
            >
              {t("management_fee.mode_quantity")}
            </Button>
          </div>

          {formData.mode === "percent" ? (
            // FEE-021 — percentage of the held quantity to remove (1% = 1_000_000 micro-percent)
            <CalcField
              id="management-fee-percent"
              data-testid="management-fee-percent"
              label={t("management_fee.form_percentage_label")}
              value={formData.percent}
              onValueChange={(v) => handleChange("percent", v)}
              placeholder={t("management_fee.form_percentage_placeholder")}
              required
            />
          ) : (
            <>
              {/* FEE-028 — the quantity the holding should hold after the fee */}
              <CalcField
                id="management-fee-resulting-quantity"
                label={t("management_fee.form_resulting_quantity_label")}
                value={formData.resultingQuantity}
                onValueChange={(v) => handleChange("resultingQuantity", v)}
                placeholder={t("management_fee.form_resulting_quantity_placeholder")}
                error={previewError ? t(previewError.key, previewError.vars) : undefined}
                required
              />
              {/* FEE-029 — what the entry removes, as the backend computed it */}
              {preview && (
                <p id="management-fee-preview" className="text-sm text-m3-on-surface-variant">
                  {t("management_fee.preview", {
                    date: formatIsoDateNumeric(formData.date, i18n.language),
                    held: microToFormattedQuantity(preview.held_quantity),
                    removed: microToFormattedQuantity(preview.removed_quantity),
                    percent: microToFormattedQuantity(preview.percent_micros),
                  })}
                </p>
              )}
            </>
          )}
        </div>

        <TextareaField
          id="management-fee-note"
          data-testid="management-fee-note"
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

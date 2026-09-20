import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { InterestEditMode, InterestHolding } from "./useInterestTransaction";
import { useInterestTransaction } from "./useInterestTransaction";

interface InterestModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** Active non-cash holdings plus the cash line interest can be credited to (INT-011/020/023). */
  heldAssets: InterestHolding[];
  onSubmitSuccess: () => void;
  /** Present when correcting an existing credit (INT-040); the asset is locked. */
  editMode?: InterestEditMode;
}

export function InterestModal({
  isOpen,
  onClose,
  accountId,
  heldAssets,
  onSubmitSuccess,
  editMode,
}: InterestModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[InterestModal] mounted");
  }, []);

  const { formData, error, isSubmitting, isFormValid, isAssetLocked, handleChange, handleSubmit } =
    useInterestTransaction({ accountId, onSubmitSuccess, editMode });

  const assetOptions = useMemo(
    () => [
      { label: t("interest.form_select_asset"), value: "" },
      ...heldAssets.map((a) => ({ label: a.assetName, value: a.assetId })),
    ],
    [heldAssets, t],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button id="interest-cancel" variant="secondary" onClick={onClose} disabled={isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="interest-form"
          id="interest-submit"
          data-testid="interest-submit"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("interest.action_record")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="interest-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t(editMode ? "interest.edit_title" : "interest.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="interest-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* INT-020 — credited asset chosen inside the modal (cash line included); locked on edit (INT-040) */}
        <SelectField
          id="interest-asset"
          data-testid="interest-asset"
          label={t("interest.form_asset_label")}
          value={formData.assetId}
          onChange={(e) => handleChange("assetId", e.target.value)}
          options={assetOptions}
          disabled={isAssetLocked}
          required
        />

        <DateField
          id="interest-date"
          data-testid="interest-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* INT-020/021 — interest rate (1% = 1_000_000 micro-percent), mutually
            exclusive with the direct quantity; a correction edits the credited
            quantity only, so the rate field is absent in edit mode (INT-040) */}
        {!editMode && (
          <CalcField
            id="interest-percent"
            data-testid="interest-percent"
            label={t("interest.form_percent_label")}
            value={formData.percent}
            onValueChange={(v) => handleChange("percent", v)}
            placeholder={t("interest.form_percent_placeholder")}
          />
        )}

        {/* INT-020/021 — directly credited quantity; no amount / price / fees */}
        <CalcField
          id="interest-quantity"
          data-testid="interest-quantity"
          label={t("interest.form_quantity_label")}
          value={formData.quantity}
          onValueChange={(v) => handleChange("quantity", v)}
          placeholder={t("interest.form_quantity_placeholder")}
        />

        <TextareaField
          id="interest-note"
          data-testid="interest-note"
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

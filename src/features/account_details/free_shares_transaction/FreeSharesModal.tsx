import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { FreeSharesEditMode, FreeSharesHolding } from "./useFreeSharesTransaction";
import { useFreeSharesTransaction } from "./useFreeSharesTransaction";

interface FreeSharesModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** Active non-cash holdings the free shares can be attributed to (FSD-011/020). */
  heldAssets: FreeSharesHolding[];
  onSubmitSuccess: () => void;
  /** Present when correcting an existing distribution (FSD-040); the asset is locked. */
  editMode?: FreeSharesEditMode;
}

export function FreeSharesModal({
  isOpen,
  onClose,
  accountId,
  heldAssets,
  onSubmitSuccess,
  editMode,
}: FreeSharesModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[FreeSharesModal] mounted");
  }, []);

  const { formData, error, isSubmitting, isFormValid, isAssetLocked, handleChange, handleSubmit } =
    useFreeSharesTransaction({ accountId, onSubmitSuccess, editMode });

  const assetOptions = useMemo(
    () => [
      { label: t("free_shares.form_select_asset"), value: "" },
      ...heldAssets.map((a) => ({ label: a.assetName, value: a.assetId })),
    ],
    [heldAssets, t],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button
          id="free-shares-cancel"
          variant="secondary"
          onClick={onClose}
          disabled={isSubmitting}
        >
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="free-shares-form"
          id="free-shares-submit"
          data-testid="free-shares-submit"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("free_shares.action_record")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="free-shares-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("free_shares.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="free-shares-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* FSD-020 — distributing asset chosen inside the modal; locked on edit (FSD-040) */}
        <SelectField
          id="free-shares-asset-select"
          data-testid="free-shares-asset-select"
          label={t("free_shares.form_asset_label")}
          value={formData.assetId}
          onChange={(e) => handleChange("assetId", e.target.value)}
          options={assetOptions}
          disabled={isAssetLocked}
          required
        />

        <DateField
          id="free-shares-date"
          data-testid="free-shares-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* FSD-020 — quantity of free shares received; no amount / price / fees */}
        <CalcField
          id="free-shares-quantity"
          data-testid="free-shares-quantity"
          label={t("free_shares.form_quantity_label")}
          value={formData.quantity}
          onValueChange={(v) => handleChange("quantity", v)}
          placeholder={t("free_shares.form_quantity_placeholder")}
          required
        />

        <TextareaField
          id="free-shares-note"
          data-testid="free-shares-note"
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

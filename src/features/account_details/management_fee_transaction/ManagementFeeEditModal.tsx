import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { ManagementFeeEditContext } from "./useManagementFeeEdit";
import { useManagementFeeEdit } from "./useManagementFeeEdit";

interface ManagementFeeEditModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** The fee deduction being corrected (FEE-063); the charged asset is locked. */
  editContext: ManagementFeeEditContext;
  onSubmitSuccess: () => void;
}

export function ManagementFeeEditModal({
  isOpen,
  onClose,
  accountId,
  editContext,
  onSubmitSuccess,
}: ManagementFeeEditModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[ManagementFeeEditModal] mounted");
  }, []);

  const { formData, error, isSubmitting, isFormValid, handleChange, handleSubmit } =
    useManagementFeeEdit({ accountId, editContext, onSubmitSuccess });

  // FEE-063 — the charged asset is immutable on edit: a single, disabled option.
  const assetOptions = useMemo(
    () => [{ label: editContext.lockedAssetName, value: editContext.lockedAssetName }],
    [editContext.lockedAssetName],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button
          id="management-fee-edit-cancel"
          variant="secondary"
          onClick={onClose}
          disabled={isSubmitting}
        >
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="management-fee-edit-form"
          id="management-fee-edit-submit"
          data-testid="management-fee-edit-submit"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("action.save")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="management-fee-edit-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("management_fee.edit_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="management-fee-edit-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* FEE-063 — charged asset shown for context but locked (immutable on edit) */}
        <SelectField
          id="management-fee-edit-asset"
          label={t("management_fee.form_asset_label")}
          value={editContext.lockedAssetName}
          onChange={() => {}}
          options={assetOptions}
          disabled
        />

        <DateField
          id="management-fee-edit-date"
          data-testid="management-fee-edit-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* FEE-063 — the removed quantity is edited directly; the percentage is not retained */}
        <CalcField
          id="management-fee-edit-quantity"
          data-testid="management-fee-edit-quantity"
          label={t("management_fee.form_quantity_label")}
          value={formData.quantity}
          onValueChange={(v) => handleChange("quantity", v)}
          placeholder={t("management_fee.form_quantity_placeholder")}
          required
        />

        <TextareaField
          id="management-fee-edit-note"
          data-testid="management-fee-edit-note"
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

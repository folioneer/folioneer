import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { formatQuantity, microToFormatted, microToFormattedPrice } from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { WriteOffTarget } from "../shared/types";
import { useWriteOff, WRITE_OFF_REASONS, type WriteOffEditMode } from "./useWriteOff";

interface WriteOffModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** The holding written off (WOF-050). */
  target: WriteOffTarget;
  onSubmitSuccess: () => void;
  /** Present when correcting a write-off from a journal (WOF-058). */
  editMode?: WriteOffEditMode;
}

export function WriteOffModal({
  isOpen,
  onClose,
  accountId,
  target,
  onSubmitSuccess,
  editMode,
}: WriteOffModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[WriteOffModal] mounted");
  }, []);

  const {
    formData,
    refusal,
    isSubmitting,
    isFormValid,
    heldQuantity,
    overHolding,
    estimatedCostMicro,
    handleChange,
    handleSubmit,
  } = useWriteOff({ accountId, target, onSubmitSuccess, editMode });

  const reasonOptions = useMemo(
    () => WRITE_OFF_REASONS.map((r) => ({ label: t(`write_off.reason_${r}`), value: r })),
    [t],
  );

  const held = formatQuantity(heldQuantity);
  // WOF-053 — over the quantity held, refused under the field; WOF-055 — a refusal on the
  // quantity shows there too.
  const quantityError = overHolding
    ? t("write_off.error_over_holding", { held, reference: target.assetReference })
    : refusal?.onQuantity
      ? t(refusal.message.key, refusal.message.vars)
      : undefined;

  const footer = (
    <div className="flex items-center justify-end gap-2">
      <Button id="write-off-cancel" variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="write-off-form"
        id="write-off-submit"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || !isFormValid || refusal !== null}
      >
        {t("write_off.action_record")}
      </Button>
    </div>
  );

  return (
    <FormModal
      id="write-off-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("write_off.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="write-off-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* WOF-051 — the asset, what is held and its average price */}
        <p id="write-off-holding" className="text-sm text-m3-on-surface">
          {t("write_off.asset_label")} <span className="font-semibold">{target.assetName}</span>
          {" · "}
          {t("write_off.holding_line", {
            quantity: held,
            reference: target.assetReference,
            price: microToFormattedPrice(target.averagePriceMicro),
          })}
        </p>

        <DateField
          id="write-off-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        <SelectField
          id="write-off-reason"
          label={t("write_off.form_reason_label")}
          value={formData.reason}
          onChange={(e) => handleChange("reason", e.target.value)}
          options={reasonOptions}
          required
        />

        <CalcField
          id="write-off-quantity"
          label={t("write_off.form_quantity_label")}
          value={formData.quantity}
          onValueChange={(v) => handleChange("quantity", v)}
          error={quantityError}
          required
        />

        {/* WOF-052 — the form's estimate at the average price shown */}
        {estimatedCostMicro !== null && refusal === null && (
          <p id="write-off-estimate" className="text-sm text-m3-on-surface-variant">
            {t(formData.reason === "Lost" ? "write_off.estimate_lost" : "write_off.estimate", {
              quantity: formData.quantity,
              reference: target.assetReference,
              cost: microToFormatted(estimatedCostMicro, 2),
            })}
          </p>
        )}

        <TextareaField
          id="write-off-note"
          label={t("transaction.form_note_label")}
          rows={2}
          value={formData.note}
          onChange={(e) => handleChange("note", e.target.value)}
          placeholder={t("transaction.form_note_placeholder")}
        />

        {refusal && !refusal.onQuantity && (
          <p id="write-off-error" role="alert" className="text-sm text-m3-error">
            {t(refusal.message.key, refusal.message.vars)}
          </p>
        )}
      </form>
    </FormModal>
  );
}

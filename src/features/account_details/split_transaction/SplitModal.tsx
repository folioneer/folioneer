import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { TextField } from "@/ui/components/field/TextField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { SplitTarget } from "../shared/types";
import type { SplitEditMode } from "./useSplitTransaction";
import { useSplitTransaction } from "./useSplitTransaction";

interface SplitModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** The holding being split — feeds the preview and the price prefill (SPL-061/040). */
  target: SplitTarget;
  onSubmitSuccess: () => void;
  /** Present when correcting an existing split (SPL-030); the asset is locked. */
  editMode?: SplitEditMode;
}

export function SplitModal({
  isOpen,
  onClose,
  accountId,
  target,
  onSubmitSuccess,
  editMode,
}: SplitModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[SplitModal] mounted");
  }, []);

  const {
    formData,
    preview,
    ratioError,
    error,
    isSubmitting,
    isFormValid,
    recordPrice,
    setRecordPrice,
    priceInput,
    handlePriceChange,
    handleChange,
    handleSubmit,
  } = useSplitTransaction({ accountId, target, onSubmitSuccess, editMode });

  const assetName = editMode?.lockedAssetName ?? target.assetName;

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button id="split-trx-cancel" variant="secondary" onClick={onClose} disabled={isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          type="submit"
          form="split-trx-form"
          id="split-trx-submit"
          data-testid="split-trx-submit"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("split.action_record")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="split-trx-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t(editMode ? "split.edit_title" : "split.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="split-trx-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* SPL-061 — the split asset is fixed by the originating holding row */}
        <p id="split-trx-asset" className="text-sm text-m3-on-surface-variant">
          {t("transaction.form_asset_label")}:{" "}
          <span className="font-medium text-m3-on-surface">{assetName}</span>
        </p>

        <DateField
          id="split-trx-date"
          data-testid="split-trx-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {editMode ? (
          /* SPL-030 — edit mode corrects the factor as a decimal multiplier */
          <CalcField
            id="split-trx-factor"
            data-testid="split-trx-factor"
            label={t("split.form_factor_label")}
            value={formData.factor}
            onValueChange={(v) => handleChange("factor", v)}
            error={
              formData.factor !== "" && ratioError ? t(ratioError.key, ratioError.vars) : undefined
            }
            required
          />
        ) : (
          <>
            {/* SPL-061 — ratio input as a new : old positive-integer pair */}
            <fieldset className="flex flex-col gap-1">
              <legend className="m3-input-label">{t("split.form_ratio_label")}</legend>
              <div className="flex items-end gap-3">
                <div className="w-28">
                  <TextField
                    id="split-trx-ratio-new"
                    data-testid="split-trx-ratio-new"
                    label={t("split.form_ratio_new_label")}
                    type="number"
                    min={1}
                    step={1}
                    value={formData.ratioNew}
                    onChange={(e) => handleChange("ratioNew", e.target.value)}
                    required
                  />
                </div>
                <span className="pb-2 text-m3-on-surface-variant">:</span>
                <div className="w-28">
                  <TextField
                    id="split-trx-ratio-old"
                    data-testid="split-trx-ratio-old"
                    label={t("split.form_ratio_old_label")}
                    type="number"
                    min={1}
                    step={1}
                    value={formData.ratioOld}
                    onChange={(e) => handleChange("ratioOld", e.target.value)}
                    required
                  />
                </div>
              </div>
              {/* SPL-011 — inline rejection on an invalid or ×1 ratio */}
              {ratioError && (
                <p role="alert" className="text-xs text-m3-error mt-1 ml-1">
                  {t(ratioError.key, ratioError.vars)}
                </p>
              )}
            </fieldset>

            {/* SPL-061 — read-only preview of the rescaled position */}
            {preview && (
              <p id="split-trx-preview" className="text-sm text-m3-on-surface-variant">
                {t("split.preview", {
                  oldQty: preview.oldQuantity,
                  oldAvg: preview.oldAveragePrice,
                  newQty: preview.newQuantity,
                  newAvg: preview.newAveragePrice,
                })}
              </p>
            )}

            {/* SPL-040 — post-split price record (best-effort on submit) */}
            <label className="flex items-center gap-3 cursor-pointer group">
              <input
                type="checkbox"
                id="split-trx-record-price"
                data-testid="split-trx-record-price"
                checked={recordPrice}
                onChange={(e) => setRecordPrice(e.target.checked)}
                className="accent-m3-primary w-4 h-4"
              />
              <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
                {t("split.record_price_label")}
              </span>
            </label>
            {recordPrice && (
              <CalcField
                id="split-trx-price"
                data-testid="split-trx-price"
                label={t("split.form_price_label")}
                value={priceInput}
                onValueChange={handlePriceChange}
              />
            )}
          </>
        )}

        <TextareaField
          id="split-trx-note"
          data-testid="split-trx-note"
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

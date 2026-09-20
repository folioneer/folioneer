import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThresholdDirection } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { HoldingNoteTarget } from "../shared/types";
import { NOTE_TEXT_MAX_LENGTH, useHoldingNote } from "./useHoldingNote";

interface HoldingNoteModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  /** The noted holding — `existing` prefills the form when a note is stored (HNO-020/042). */
  target: HoldingNoteTarget;
  onSubmitSuccess: () => void;
}

export function HoldingNoteModal({
  isOpen,
  onClose,
  accountId,
  target,
  onSubmitSuccess,
}: HoldingNoteModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[HoldingNoteModal] mounted");
  }, []);

  const {
    formData,
    isEditMode,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
    handleDelete,
  } = useHoldingNote({ accountId, target, onSubmitSuccess });
  // HNO-021 — destructive action goes through the house confirm step.
  const [confirmDelete, setConfirmDelete] = useState(false);

  const directionOptions = useMemo(
    () => [
      { label: t("holding_note.direction_below"), value: "Below" },
      { label: t("holding_note.direction_above"), value: "Above" },
    ],
    [t],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-between gap-2">
        {/* HNO-021/042 — delete only offered on an existing note, destructive style */}
        {isEditMode ? (
          <Button
            id="holding-note-delete"
            data-testid="holding-note-delete"
            variant="danger"
            onClick={() => setConfirmDelete(true)}
            disabled={isSubmitting}
          >
            {t("holding_note.action_delete")}
          </Button>
        ) : (
          <span />
        )}
        <div className="flex items-center gap-2">
          <Button
            id="holding-note-cancel"
            variant="secondary"
            onClick={onClose}
            disabled={isSubmitting}
          >
            {t("action.cancel")}
          </Button>
          <Button
            type="submit"
            form="holding-note-form"
            id="holding-note-submit"
            data-testid="holding-note-submit"
            variant="primary"
            loading={isSubmitting}
            disabled={isSubmitting || !isFormValid}
          >
            {t("holding_note.action_save")}
          </Button>
        </div>
      </div>
    ),
    [isEditMode, isSubmitting, isFormValid, t, onClose],
  );

  return (
    <FormModal
      id="holding-note-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("holding_note.modal_title", { asset: target.assetName })}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="holding-note-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* HNO-011 — 1-500 chars; maxLength caps typing, the hook gates over-length */}
        <TextareaField
          id="holding-note-text"
          data-testid="holding-note-text"
          label={t("holding_note.form_text_label")}
          rows={4}
          autoFocus
          maxLength={NOTE_TEXT_MAX_LENGTH}
          value={formData.text}
          onChange={(e) => handleChange("text", e.target.value)}
          placeholder={t("holding_note.form_text_placeholder")}
          required
        />

        {/* HNO-042 — "alert me" toggle revealing the alarm pair (direction + threshold) */}
        <label className="flex items-center gap-3 cursor-pointer group">
          <input
            type="checkbox"
            id="holding-note-alarm-toggle"
            data-testid="holding-note-alarm-toggle"
            checked={formData.alarmEnabled}
            onChange={(e) => handleChange("alarmEnabled", e.target.checked)}
            className="accent-m3-primary w-4 h-4"
          />
          <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
            {t("holding_note.alarm_toggle_label")}
          </span>
        </label>

        {formData.alarmEnabled && (
          <>
            {/* HNO-030 — Below / Above relative to the threshold */}
            <SelectField
              id="holding-note-direction"
              data-testid="holding-note-direction"
              label={t("holding_note.form_direction_label")}
              value={formData.direction}
              onChange={(e) => handleChange("direction", e.target.value as ThresholdDirection)}
              options={directionOptions}
            />
            {/* HNO-031 — nominal share price in the asset's currency */}
            <CalcField
              id="holding-note-price"
              data-testid="holding-note-price"
              label={t("holding_note.form_price_label", { currency: target.assetCurrency })}
              value={formData.price}
              onValueChange={(v) => handleChange("price", v)}
              required
            />
          </>
        )}

        {/* F27 — backend rejection surfaced inline */}
        {error && (
          <p role="alert" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
      {/* HNO-021 — confirm before the destructive delete (house precedent) */}
      <ConfirmationDialog
        confirmId="holding-note-delete-confirm"
        isOpen={confirmDelete}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={() => {
          setConfirmDelete(false);
          void handleDelete();
        }}
        title={t("holding_note.delete_confirm_title")}
        message={t("holding_note.delete_confirm_message")}
        confirmLabel={t("action.delete")}
        cancelLabel={t("action.cancel")}
      />
    </FormModal>
  );
}

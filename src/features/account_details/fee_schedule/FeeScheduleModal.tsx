import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { FeeFrequency } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { FormModal } from "@/ui/components/modal/FormModal";
import { useFeeSchedule } from "./useFeeSchedule";

interface FeeScheduleModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  assetId: string;
  assetName: string;
  onSubmitSuccess: () => void;
}

const FREQUENCIES: FeeFrequency[] = ["Monthly", "Quarterly", "Annually"];

export function FeeScheduleModal({
  isOpen,
  onClose,
  accountId,
  assetId,
  assetName,
  onSubmitSuccess,
}: FeeScheduleModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[FeeScheduleModal] mounted");
  }, []);

  const {
    formData,
    isExisting,
    isLoading,
    error,
    isSubmitting,
    isFormValid,
    handleChange,
    handleSubmit,
    handleDelete,
  } = useFeeSchedule({ accountId, assetId, onSubmitSuccess });

  // FEE-062 — deleting a schedule is destructive; gate it behind a confirmation.
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const frequencyOptions = useMemo(
    () =>
      FREQUENCIES.map((f) => ({
        label: t(`fee_schedule.frequency_${f.toLowerCase()}`),
        value: f,
      })),
    [t],
  );

  const statusOptions = useMemo(
    () => [
      { label: t("fee_schedule.status_active"), value: "active" },
      { label: t("fee_schedule.status_paused"), value: "paused" },
    ],
    [t],
  );

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-between gap-2">
        {isExisting ? (
          <Button
            id="fee-schedule-delete"
            data-testid="fee-schedule-delete"
            variant="danger"
            onClick={() => setShowDeleteConfirm(true)}
            disabled={isSubmitting || isLoading}
          >
            {t("fee_schedule.action_delete")}
          </Button>
        ) : (
          <span />
        )}
        <div className="flex items-center gap-2">
          <Button
            id="fee-schedule-cancel"
            variant="secondary"
            onClick={onClose}
            disabled={isSubmitting}
          >
            {t("action.cancel")}
          </Button>
          <Button
            type="submit"
            form="fee-schedule-form"
            id="fee-schedule-submit"
            data-testid="fee-schedule-submit"
            variant="primary"
            loading={isSubmitting}
            disabled={isSubmitting || isLoading || !isFormValid}
          >
            {t("fee_schedule.action_save")}
          </Button>
        </div>
      </div>
    ),
    [isExisting, isSubmitting, isLoading, isFormValid, t, onClose],
  );

  return (
    <>
      <FormModal
        id="fee-schedule-modal"
        isOpen={isOpen}
        onClose={onClose}
        title={t("fee_schedule.modal_title", { asset: assetName })}
        footer={footer}
        maxWidth="max-w-2xl"
      >
        <form id="fee-schedule-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
          {/* FEE-032 — annual rate as a percentage (1% = 1_000_000 micro-percent) */}
          <CalcField
            id="fee-schedule-rate"
            data-testid="fee-schedule-rate"
            label={t("fee_schedule.form_rate_label")}
            value={formData.ratePercent}
            onValueChange={(v) => handleChange("ratePercent", v)}
            placeholder={t("fee_schedule.form_rate_placeholder")}
            required
          />

          {/* FEE-034/060 — cadence is immutable once the schedule exists */}
          <SelectField
            id="fee-schedule-frequency"
            data-testid="fee-schedule-frequency"
            label={t("fee_schedule.form_frequency_label")}
            value={formData.frequency}
            onChange={(e) => handleChange("frequency", e.target.value as FeeFrequency)}
            options={frequencyOptions}
            disabled={isExisting}
            required
          />

          {/* FEE-032/060 — start date is immutable once the schedule exists */}
          <DateField
            id="fee-schedule-start-date"
            data-testid="fee-schedule-start-date"
            label={t("fee_schedule.form_start_date_label")}
            value={formData.startDate}
            onChange={(e) => handleChange("startDate", e.target.value)}
            disabled={isExisting}
            required
          />

          {/* FEE-045 — optional end date after which generation stops */}
          <DateField
            id="fee-schedule-end-date"
            data-testid="fee-schedule-end-date"
            label={t("fee_schedule.form_end_date_label")}
            value={formData.endDate}
            onChange={(e) => handleChange("endDate", e.target.value)}
          />

          {/* FEE-061 — pausing keeps the schedule but stops generation */}
          {isExisting && (
            <SelectField
              id="fee-schedule-status"
              data-testid="fee-schedule-status"
              label={t("fee_schedule.form_status_label")}
              value={formData.active ? "active" : "paused"}
              onChange={(e) => handleChange("active", e.target.value === "active")}
              options={statusOptions}
            />
          )}

          {error && (
            <p role="alert" className="text-sm text-m3-error">
              {t(error.key, error.vars)}
            </p>
          )}
        </form>
      </FormModal>

      {/* FEE-062 — confirm before removing a recurring schedule */}
      <ConfirmationDialog
        isOpen={showDeleteConfirm}
        onCancel={() => setShowDeleteConfirm(false)}
        onConfirm={() => {
          setShowDeleteConfirm(false);
          void handleDelete();
        }}
        title={t("fee_generation.confirm_delete_title")}
        message={t("fee_generation.confirm_delete_message")}
        confirmLabel={t("fee_schedule.action_delete")}
        cancelLabel={t("action.cancel")}
        variant="danger"
        confirmId="fee-schedule-delete-confirm"
      />
    </>
  );
}

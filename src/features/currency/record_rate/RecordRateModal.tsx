import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import type { CurrencyRate } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { DateField } from "@/ui/components/field/DateField";
import { TextField } from "@/ui/components/field/TextField";
import { Dialog } from "@/ui/components/modal/Dialog";
import type { I18nMessage } from "@/ui/format/i18n";
import { deleteCurrencyRate } from "../gateway";
import { currencyErrorToI18n, validateRateForm } from "../shared/presenter";
import { useRecordRate } from "./useRecordRate";

interface RecordRateModalProps {
  isOpen: boolean;
  fromCurrency: string;
  toCurrency: string;
  /** Edit mode (FXR-052): pre-fills date + rate and routes the submit to `updateCurrencyRate`. */
  initialRate?: CurrencyRate;
  onClose: () => void;
  onSuccess: () => void;
}

/** FXR-025/052 — record a new rate (create) or edit an existing one. */
export function RecordRateModal({
  isOpen,
  fromCurrency,
  toCurrency,
  initialRate,
  onClose,
  onSuccess,
}: RecordRateModalProps) {
  const { t } = useTranslation();
  const { date, rate, setDate, setRate, isSubmitting, error, isEditMode, submit } = useRecordRate({
    fromCurrency,
    toCurrency,
    initialRate,
    onSuccess,
  });

  // FXR-020–023 — non-blocking inline hints mirroring the domain guards. The
  // hints surface immediately, but submit still round-trips to the gateway
  // (the backend is the source of truth for rejection).
  const validation = validateRateForm({ fromCurrency, toCurrency, date, rate });
  // Only hint on a field the user has touched, so a pristine form stays clean.
  const dateHint = date.trim() !== "" ? validation.errors.date : undefined;
  const rateHint = rate.trim() !== "" ? validation.errors.rate : undefined;

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="button"
        variant="primary"
        id="record-rate-submit"
        data-testid="record-rate-submit"
        loading={isSubmitting}
        disabled={isSubmitting}
        onClick={() => void submit()}
      >
        {t("action.save")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="record-rate-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={
        isEditMode
          ? t("currency.edit_rate_title", { from: fromCurrency, to: toCurrency })
          : t("currency.record_rate_title", { from: fromCurrency, to: toCurrency })
      }
      actions={actions}
    >
      <div className="flex flex-col gap-4 py-2">
        <div className="flex flex-col gap-1">
          <DateField
            id="record-rate-date"
            data-testid="record-rate-date"
            label={t("currency.form_date_label")}
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
          {dateHint && (
            <p
              id="record-rate-date-hint"
              data-testid="record-rate-date-hint"
              className="text-xs text-m3-error"
            >
              {t(dateHint)}
            </p>
          )}
        </div>
        <div className="flex flex-col gap-1">
          <TextField
            id="record-rate-rate"
            data-testid="record-rate-rate"
            type="text"
            inputMode="decimal"
            label={t("currency.form_rate_label", { from: fromCurrency, to: toCurrency })}
            value={rate}
            placeholder={t("currency.form_rate_placeholder")}
            onChange={(e) => setRate(e.target.value)}
          />
          {rateHint && (
            <p
              id="record-rate-rate-hint"
              data-testid="record-rate-rate-hint"
              className="text-xs text-m3-error"
            >
              {t(rateHint)}
            </p>
          )}
        </div>
        {error && (
          <p role="alert" data-testid="record-rate-error" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </div>
    </Dialog>
  );
}

interface DeleteRateConfirmationProps {
  isOpen: boolean;
  rate: CurrencyRate;
  onClose: () => void;
  onSuccess: () => void;
}

/** FXR-053 — confirm-and-delete dialog for a single dated rate. */
export function DeleteRateConfirmation({
  isOpen,
  rate,
  onClose,
  onSuccess,
}: DeleteRateConfirmationProps) {
  const { t } = useTranslation();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  const handleConfirm = useCallback(async () => {
    setIsSubmitting(true);
    setError(null);
    const result = await deleteCurrencyRate(rate.from_currency, rate.to_currency, rate.date);
    if (result.status === "ok") {
      onSuccess();
    } else {
      setError(currencyErrorToI18n(result.error));
    }
    setIsSubmitting(false);
  }, [rate.from_currency, rate.to_currency, rate.date, onSuccess]);

  const actions = (
    <>
      <Button variant="secondary" data-testid="delete-rate-cancel" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="button"
        variant="danger"
        id="delete-rate-confirm"
        data-testid="delete-rate-confirm"
        loading={isSubmitting}
        disabled={isSubmitting}
        onClick={() => void handleConfirm()}
      >
        {t("action.delete")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="delete-rate-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("currency.delete_rate_title")}
      actions={actions}
    >
      <p className="text-m3-on-surface-variant">
        {t("currency.delete_rate_message", {
          from: rate.from_currency,
          to: rate.to_currency,
          date: rate.date,
        })}
      </p>
      {error && (
        <p role="alert" data-testid="delete-rate-error" className="mt-3 text-sm text-m3-error">
          {t(error.key, error.vars)}
        </p>
      )}
    </Dialog>
  );
}

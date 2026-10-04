import { useCallback, useState } from "react";
import type { CurrencyRate } from "@/bindings";
import {
  decimalToDisplayed,
  decimalToNumber,
  microToFieldDecimal,
  typedToDecimal,
} from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { recordCurrencyRate, updateCurrencyRate } from "../gateway";
import { currencyErrorToI18n } from "../shared/presenter";

interface UseRecordRateArgs {
  fromCurrency: string;
  toCurrency: string;
  /** When present, the form runs in edit mode and calls `updateCurrencyRate` (FXR-052). */
  initialRate?: CurrencyRate;
  onSuccess: () => void;
}

interface UseRecordRateResult {
  date: string;
  rate: string;
  setDate: (value: string) => void;
  setRate: (value: string) => void;
  isSubmitting: boolean;
  error: I18nMessage | null;
  isEditMode: boolean;
  submit: () => Promise<void>;
}

/** FXR-025/052 — record (create) or update (edit) a manual currency rate. */
export function useRecordRate({
  fromCurrency,
  toCurrency,
  initialRate,
  onSuccess,
}: UseRecordRateArgs): UseRecordRateResult {
  const showSnackbar = useSnackbar();
  const isEditMode = initialRate !== undefined;
  const [date, setDate] = useState(initialRate?.date ?? "");
  // NUM-010/011 — the field shows the decimal separator of the display language, a rate
  // being corrected with every decimal recorded (TRX-024).
  const [rate, setRateText] = useState(
    initialRate !== undefined ? decimalToDisplayed(microToFieldDecimal(initialRate.rate)) : "",
  );
  const setRate = useCallback(
    (typed: string) => setRateText(decimalToDisplayed(typedToDecimal(typed))),
    [],
  );
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  const submit = useCallback(async () => {
    setIsSubmitting(true);
    setError(null);
    const rateValue = decimalToNumber(typedToDecimal(rate));

    const result =
      initialRate !== undefined
        ? await updateCurrencyRate(fromCurrency, toCurrency, initialRate.date, date, rateValue)
        : await recordCurrencyRate(fromCurrency, toCurrency, date, rateValue);

    if (result.status === "ok") {
      showSnackbar(isEditMode ? "currency.rate_updated" : "currency.rate_recorded", "success");
      onSuccess();
    } else {
      setError(currencyErrorToI18n(result.error));
    }
    setIsSubmitting(false);
  }, [fromCurrency, toCurrency, date, rate, initialRate, isEditMode, onSuccess, showSnackbar]);

  return { date, rate, setDate, setRate, isSubmitting, error, isEditMode, submit };
}

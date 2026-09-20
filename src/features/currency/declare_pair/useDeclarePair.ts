import { useCallback, useState } from "react";
import type { I18nMessage } from "@/ui/format/i18n";
import { declareCurrencyPair } from "../gateway";
import { currencyErrorToI18n } from "../shared/presenter";

interface UseDeclarePairArgs {
  onSuccess: () => void;
}

interface UseDeclarePairResult {
  fromCurrency: string;
  toCurrency: string;
  setFromCurrency: (value: string) => void;
  setToCurrency: (value: string) => void;
  isSubmitting: boolean;
  error: I18nMessage | null;
  /** FXR-055 — true while the pair cannot yet be submitted (empty or identical). */
  isSubmitDisabled: boolean;
  submit: () => Promise<void>;
}

/** FXR-054/055 — declare-a-currency-pair form state + submit. */
export function useDeclarePair({ onSuccess }: UseDeclarePairArgs): UseDeclarePairResult {
  const [fromCurrency, setFromCurrency] = useState("");
  const [toCurrency, setToCurrency] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  const from = fromCurrency.trim().toUpperCase();
  const to = toCurrency.trim().toUpperCase();
  const isSubmitDisabled = from === "" || to === "" || from === to;

  const submit = useCallback(async () => {
    if (isSubmitDisabled) return;
    setIsSubmitting(true);
    setError(null);
    const result = await declareCurrencyPair(from, to);
    if (result.status === "ok") {
      onSuccess();
    } else {
      setError(currencyErrorToI18n(result.error));
    }
    setIsSubmitting(false);
  }, [from, to, isSubmitDisabled, onSuccess]);

  return {
    fromCurrency,
    toCurrency,
    setFromCurrency,
    setToCurrency,
    isSubmitting,
    error,
    isSubmitDisabled,
    submit,
  };
}

import { useCallback, useEffect, useState } from "react";
import type { I18nMessage } from "@/ui/format/i18n";
import {
  backfillCurrencyRateHistory,
  getReferenceCurrency,
  setReferenceCurrency,
} from "../gateway";
import { rateHistoryErrorToI18n, referenceCurrencyErrorToI18n } from "../shared/presenter";

export interface UseReferenceCurrencyResult {
  /** The reference currency; null until read. */
  currency: string | null;
  /** Every code it may be set to (FXR-127). */
  choices: string[];
  /** FXR-126 — true while the rate histories the new currency needs are fetched. */
  isFetchingHistory: boolean;
  /** True from a choice until it is stored and its histories fetched: no second choice. */
  isBusy: boolean;
  /** A failed read, choice or fetch; the currency reverts on a failed choice. */
  error: I18nMessage | null;
  choose: (currency: string) => Promise<void>;
}

/**
 * GPF-011 / FXR-129–131 — the Settings choice of the reference currency: reads it with its
 * choices; a choice is stored, then the rate histories are fetched (FXR-015/112) while the
 * section says so. Every total re-reads on the event the choice publishes.
 */
export function useReferenceCurrency(): UseReferenceCurrencyResult {
  const [currency, setCurrency] = useState<string | null>(null);
  const [choices, setChoices] = useState<string[]>([]);
  const [isFetchingHistory, setIsFetchingHistory] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const result = await getReferenceCurrency();
      if (cancelled) return;
      if (result.status === "ok") {
        setCurrency(result.data.currency);
        setChoices(result.data.choices);
      } else {
        setError(referenceCurrencyErrorToI18n(result.error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const choose = useCallback(
    async (next: string) => {
      const prior = currency;
      setIsBusy(true);
      setError(null);
      setCurrency(next);
      const result = await setReferenceCurrency(next);
      if (result.status === "error") {
        setCurrency(prior);
        setError(referenceCurrencyErrorToI18n(result.error));
        setIsBusy(false);
        return;
      }
      setIsFetchingHistory(true);
      const fetched = await backfillCurrencyRateHistory();
      setIsFetchingHistory(false);
      setIsBusy(false);
      if (fetched.status === "error") {
        setError(rateHistoryErrorToI18n(fetched.error));
      }
    },
    [currency],
  );

  return { currency, choices, isFetchingHistory, isBusy, error, choose };
}

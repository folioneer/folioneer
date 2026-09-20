import { useCallback, useEffect, useState } from "react";
import type { CurrencyPairSummary, CurrencyRate } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import {
  backfillCurrencyRateHistory,
  getCurrencyPairs,
  getCurrencyRates,
  subscribeToEvents,
} from "../gateway";
import { currencyErrorToI18n, rateHistoryBackfillErrorToI18n } from "../shared/presenter";

interface SelectedPair {
  fromCurrency: string;
  toCurrency: string;
}

interface UseCurrencyRatesViewResult {
  isLoading: boolean;
  error: I18nMessage | null;
  pairs: CurrencyPairSummary[];
  selectedPair: SelectedPair | null;
  rates: CurrencyRate[];
  ratesError: I18nMessage | null;
  selectPair: (fromCurrency: string, toCurrency: string) => void;
  clearSelection: () => void;
  refetch: () => void;
  /** FXR-110 — true while the history backfill is being acknowledged. */
  isBackfilling: boolean;
  /** FXR-110 — downloads the full rate history; resolves with the outcome. */
  backfillHistory: () => Promise<
    { status: "ok"; ratesWritten: number } | { status: "error"; message: I18nMessage }
  >;
}

/** FXR-050/051 — loads the followed pairs and, on drill-in, one pair's rate history. */
export function useCurrencyRatesView(): UseCurrencyRatesViewResult {
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [pairs, setPairs] = useState<CurrencyPairSummary[]>([]);
  const [selectedPair, setSelectedPair] = useState<SelectedPair | null>(null);
  const [rates, setRates] = useState<CurrencyRate[]>([]);
  const [ratesError, setRatesError] = useState<I18nMessage | null>(null);
  const [isBackfilling, setIsBackfilling] = useState(false);

  const fetchPairs = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    const result = await getCurrencyPairs();
    if (result.status === "ok") {
      setPairs(result.data);
    } else {
      logger.error("[useCurrencyRatesView] fetchPairs failed", result.error);
      setError(currencyErrorToI18n(result.error));
    }
    setIsLoading(false);
  }, []);

  const fetchRates = useCallback(async (fromCurrency: string, toCurrency: string) => {
    setRatesError(null);
    const result = await getCurrencyRates(fromCurrency, toCurrency);
    if (result.status === "ok") {
      setRates(result.data);
    } else {
      logger.error("[useCurrencyRatesView] fetchRates failed", result.error);
      setRatesError(currencyErrorToI18n(result.error));
    }
  }, []);

  useEffect(() => {
    void fetchPairs();
  }, [fetchPairs]);

  // FXR-026/037/054 — re-fetch when a rate or a pair is recorded/updated/deleted
  // here, and once when a sync applied changes from another device (SYN-064).
  useEffect(() => {
    const unlistenPromise = subscribeToEvents((type) => {
      if (
        type === "CurrencyRateUpdated" ||
        type === "CurrencyPairUpdated" ||
        type === "SyncCompleted"
      ) {
        void fetchPairs();
        setSelectedPair((current) => {
          if (current) void fetchRates(current.fromCurrency, current.toCurrency);
          return current;
        });
      }
    });
    return () => {
      void Promise.resolve(unlistenPromise).then((unlisten) => unlisten?.());
    };
  }, [fetchPairs, fetchRates]);

  const selectPair = useCallback(
    (fromCurrency: string, toCurrency: string) => {
      setSelectedPair({ fromCurrency, toCurrency });
      void fetchRates(fromCurrency, toCurrency);
    },
    [fetchRates],
  );

  const clearSelection = useCallback(() => {
    setSelectedPair(null);
    setRates([]);
    setRatesError(null);
  }, []);

  // FXR-110 — user-triggered full-history download; the view refreshes via
  // the caller's snackbar path + the CurrencyRateUpdated re-fetch above.
  const backfillHistory = useCallback(async () => {
    setIsBackfilling(true);
    const result = await backfillCurrencyRateHistory();
    setIsBackfilling(false);
    if (result.status === "ok") {
      void fetchPairs();
      setSelectedPair((current) => {
        if (current) void fetchRates(current.fromCurrency, current.toCurrency);
        return current;
      });
      return { status: "ok" as const, ratesWritten: result.data };
    }
    logger.error("[useCurrencyRatesView] backfillHistory failed", result.error);
    return { status: "error" as const, message: rateHistoryBackfillErrorToI18n(result.error) };
  }, [fetchPairs, fetchRates]);

  return {
    isLoading,
    error,
    pairs,
    selectedPair,
    rates,
    ratesError,
    selectPair,
    clearSelection,
    refetch: () => void fetchPairs(),
    isBackfilling,
    backfillHistory,
  };
}

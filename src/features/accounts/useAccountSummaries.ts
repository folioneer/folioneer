import { useCallback, useEffect, useRef, useState } from "react";
import type { AccountSummary, PortfolioTotal } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountGateway } from "./gateway";
import { accountMutationErrorToI18n } from "./shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

interface UseAccountSummariesResult {
  summaries: AccountSummary[];
  /** ACC-027 — the backend's portfolio total; null until the first successful read. */
  portfolioTotal: PortfolioTotal | null;
  isLoading: boolean;
  error: I18nMessage | null;
  refetch: () => Promise<void>;
}

/**
 * Fetches the per-account global-value view (ACC-021). Listens to backend
 * events so the Accounts list refreshes when account state or prices change.
 */
export function useAccountSummaries(): UseAccountSummariesResult {
  const [summaries, setSummaries] = useState<AccountSummary[]>([]);
  const [portfolioTotal, setPortfolioTotal] = useState<PortfolioTotal | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);

  // F29 — only the answer to the latest request is shown; an earlier one arriving late is dropped.
  const latestRequest = useRef(0);

  const fetchSummaries = useCallback(async () => {
    const request = ++latestRequest.current;
    setError(null);
    try {
      const result = await accountGateway.getAccountSummaries();
      if (request !== latestRequest.current) return;
      if (result.status === "ok") {
        setSummaries(result.data.summaries);
        setPortfolioTotal(result.data.total);
      } else {
        logger.error("[useAccountSummaries] fetch failed", { error: result.error });
        setError(accountMutationErrorToI18n(result.error));
      }
    } catch (err) {
      if (request !== latestRequest.current) return;
      logger.error("[useAccountSummaries] fetch threw", { error: err });
      setError(UNKNOWN_ERROR);
    } finally {
      if (request === latestRequest.current) setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    // F29 — the loading state shows for the first load and a change of key only;
    // a re-fetch after a change keeps the current rows on screen.
    setIsLoading(true);
    fetchSummaries();
  }, [fetchSummaries]);

  // Re-fetch on events that can change account values: AccountUpdated covers
  // CRUD + transaction-driven holding changes (TRX-037); AssetPriceUpdated
  // covers value drift from manual price entries + auto-fetch (MKT-036);
  // AssetUpdated covers asset currency changes that flip the same-currency filter;
  // CurrencyRateUpdated / CurrencyPairUpdated move the converted values and the
  // portfolio total (ACC-033); SyncCompleted covers everything a sync applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = accountGateway.subscribeToEvents((type) => {
      // MKT-181 — coalesce per-asset events while a bulk price fetch runs.
      if (type === "AssetPriceUpdated" && useAppStore.getState().priceFetch.active) {
        return;
      }
      if (
        type === "AccountUpdated" ||
        type === "AssetUpdated" ||
        type === "CurrencyRateUpdated" ||
        type === "CurrencyPairUpdated" ||
        type === "AssetPriceUpdated" ||
        type === "AssetPriceFetchCompleted" ||
        type === "TransactionUpdated" ||
        type === "SyncCompleted"
      ) {
        fetchSummaries();
      }
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchSummaries]);

  // F29 — a retry the user asks for shows the loading state; a re-fetch after a change does not.
  const reloadSummaries = useCallback(() => {
    setIsLoading(true);
    return fetchSummaries();
  }, [fetchSummaries]);

  return { summaries, portfolioTotal, isLoading, error, refetch: reloadSummaries };
}

import { useCallback, useEffect, useMemo, useState } from "react";
import type { AccountDetailsResponse, HoldingDetail } from "@/bindings";
import { accountMutationErrorToI18n } from "@/features/accounts/shared/presenter";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway, useCachedAssets } from "../gateway";
import {
  type AccountSummaryViewModel,
  type ClosedHoldingRowViewModel,
  type HoldingRowViewModel,
  toAccountSummary,
  toClosedHoldingRow,
  toHoldingRow,
} from "../shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

interface UseAccountDetailsResult {
  isLoading: boolean;
  error: I18nMessage | null;
  retry: () => void;
  holdings: HoldingRowViewModel[];
  /** Raw active HoldingDetail records — used to pass to PriceModal (MKT-013). */
  holdingDetails: HoldingDetail[];
  closedHoldings: ClosedHoldingRowViewModel[];
  summary: AccountSummaryViewModel | null;
}

/**
 * @param asOfDate "" for the live view (today), or an ISO "YYYY-MM-DD" to load a
 * read-only reconstruction of the account as it stood on that past date.
 */
export function useAccountDetails(accountId: string, asOfDate = ""): UseAccountDetailsResult {
  const [data, setData] = useState<AccountDetailsResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);
  // ACD-051 — asset class is read from the loaded asset catalog to group holdings.
  const assets = useCachedAssets();

  const fetchDetails = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await accountDetailsGateway.getAccountDetails(accountId, asOfDate || null);
      if (result.status === "ok") {
        setData(result.data);
      } else {
        logger.error("[useAccountDetails] fetch failed", result.error);
        setError(accountMutationErrorToI18n(result.error));
      }
    } catch (err) {
      logger.error("[useAccountDetails] fetch threw", { error: err });
      setError(UNKNOWN_ERROR);
    } finally {
      setIsLoading(false);
    }
  }, [accountId, asOfDate]);

  // ACD-037 — fetch on mount and on accountId change
  useEffect(() => {
    fetchDetails();
  }, [fetchDetails]);

  // ACD-039/040/MKT-036/FXR-036/FEE-064/HNO-020 — re-fetch on AccountUpdated (the
  // header renders the account's own name), TransactionUpdated, AssetUpdated,
  // AssetPriceUpdated, CurrencyRateUpdated, CurrencyPairUpdated (a removed pair
  // takes its rates with it), FeeScheduleUpdated, or HoldingNoteUpdated — each
  // raised by a local write — and once on SyncCompleted, the single event a sync
  // raises for everything it applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = accountDetailsGateway.subscribeToEvents((type) => {
      // MKT-181 — while a bulk price fetch runs, per-asset AssetPriceUpdated
      // events are coalesced: the view reloads once on AssetPriceFetchCompleted.
      if (type === "AssetPriceUpdated" && useAppStore.getState().priceFetch.active) {
        return;
      }
      if (
        type === "AccountUpdated" ||
        type === "TransactionUpdated" ||
        type === "AssetUpdated" ||
        type === "AssetPriceUpdated" ||
        type === "AssetPriceFetchCompleted" ||
        type === "CurrencyRateUpdated" ||
        type === "CurrencyPairUpdated" ||
        type === "FeeScheduleUpdated" ||
        type === "HoldingNoteUpdated" ||
        type === "SyncCompleted"
      ) {
        fetchDetails();
      }
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchDetails]);

  const holdingDetails = useMemo<HoldingDetail[]>(() => data?.holdings ?? [], [data]);

  // ACD-051 / CSH-092 — group active holdings by asset class: cash first, then
  // Stocks, then every other class; alphabetical by asset_name within each group.
  const holdings = useMemo<HoldingRowViewModel[]>(() => {
    if (!data) return [];
    const classById = new Map(assets.map((a) => [a.id, a.class]));
    const groupRank = (row: HoldingRowViewModel): number => {
      if (row.isCash) return 0;
      return classById.get(row.assetId) === "Stocks" ? 1 : 2;
    };
    return data.holdings
      .map((detail) => toHoldingRow(detail, data.total_global_value))
      .sort(
        (a, b) =>
          groupRank(a) - groupRank(b) ||
          a.assetName.toLowerCase().localeCompare(b.assetName.toLowerCase()),
      );
  }, [data, assets]);

  const closedHoldings = useMemo<ClosedHoldingRowViewModel[]>(
    () => (data ? data.closed_holdings.map(toClosedHoldingRow) : []),
    [data],
  );

  const summary = useMemo<AccountSummaryViewModel | null>(
    () => (data ? toAccountSummary(data) : null),
    [data],
  );

  return {
    isLoading,
    error,
    retry: fetchDetails,
    holdings,
    holdingDetails,
    closedHoldings,
    summary,
  };
}

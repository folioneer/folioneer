import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AccountPerformanceResponse } from "@/bindings";
import { logger } from "@/lib/logger";
import { getPerfViewMode, setPerfViewMode } from "@/lib/perfViewModeStorage";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountPerformanceGateway } from "../gateway";
import {
  type AssetScopeOption,
  type PerformanceViewMode,
  type PeriodRowViewModel,
  presentAccountPerformanceError,
  presentAssetScopeOptions,
  presentPeriodRow,
  presentValueChartSeries,
  resolveViewMode,
  type ValueChartPoint,
} from "../shared/presenter";

interface UseAccountPerformanceResult {
  isLoading: boolean;
  error: I18nMessage | null;
  retry: () => void;
  /** True only for Automatic/ManualDay/ManualWeek accounts (PRF-013). */
  monthViewAvailable: boolean;
  isEmpty: boolean;
  viewMode: PerformanceViewMode;
  setViewMode: (mode: PerformanceViewMode) => void;
  /** Years available in the monthly data, most-recent first (PRF-015). */
  availableYears: number[];
  selectedYear: number | null;
  setSelectedYear: (year: number) => void;
  /** Rows for the active view: yearly rows in year view, the selected year's months in month view. */
  rows: PeriodRowViewModel[];
  /** Account-value-over-time series for the line chart, chronological (oldest→newest). */
  chartPoints: ValueChartPoint[];
  /** Active non-cash holdings selectable as an asset scope (PRF-080, PRF-082). */
  assetOptions: AssetScopeOption[];
  /** The scoped asset, or null for the whole account (PRF-080). Session-scoped, per account. */
  selectedAssetId: string | null;
  setSelectedAssetId: (assetId: string | null) => void;
  /** Display name of the scoped asset; null when the whole account is shown. */
  selectedAssetName: string | null;
}

/** The asset scope keyed by account, so a navigation to another account reads as unscoped. */
interface AssetScope {
  accountId: string;
  assetId: string | null;
}

export function useAccountPerformance(accountId: string): UseAccountPerformanceResult {
  const [data, setData] = useState<AccountPerformanceResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [viewMode, setViewMode] = useState<PerformanceViewMode>("year");
  const [selectedYear, setSelectedYear] = useState<number | null>(null);
  const [assetScope, setAssetScope] = useState<AssetScope>({ accountId, assetId: null });
  const [assetOptions, setAssetOptions] = useState<AssetScopeOption[]>([]);
  const showSnackbar = useSnackbar();
  const { t } = useTranslation();
  // Monotonic request token: only the latest fetchPerformance invocation may
  // commit its response, so an older in-flight read never clobbers a newer one.
  const requestSeqRef = useRef(0);

  // PRF-080 — the scope only applies to the account it was chosen for; a stale
  // scope from a previously viewed account reads as "All assets".
  const selectedAssetId = assetScope.accountId === accountId ? assetScope.assetId : null;

  const setSelectedAssetId = useCallback(
    (assetId: string | null) => {
      setAssetScope({ accountId, assetId });
    },
    [accountId],
  );

  const fetchPerformance = useCallback(async () => {
    const requestSeq = ++requestSeqRef.current;
    setIsLoading(true);
    setError(null);
    try {
      const result = await accountPerformanceGateway.getAccountPerformance(
        accountId,
        selectedAssetId,
      );
      // A newer fetch superseded this one while it was in flight — drop the response.
      if (requestSeq !== requestSeqRef.current) return;
      if (result.status === "ok") {
        setData(result.data);
        // PRF-014 — restore the account's remembered view mode (clamped to availability),
        // falling back to the default when there is no stored preference.
        setViewMode(resolveViewMode(getPerfViewMode(accountId), result.data.month_view_available));
        // PRF-015 — default the year selector to the most-recent year in the monthly data.
        const firstMonthlyYear = result.data.monthly[0]?.year ?? null;
        setSelectedYear(firstMonthlyYear);
      } else {
        logger.error("[useAccountPerformance] fetch failed", result.error);
        setError(presentAccountPerformanceError(result.error));
      }
    } catch (err) {
      if (requestSeq !== requestSeqRef.current) return;
      logger.error("[useAccountPerformance] fetch threw", { error: err });
      setError({ key: "account_performance.error.database_error" });
    } finally {
      if (requestSeq === requestSeqRef.current) {
        setIsLoading(false);
      }
    }
  }, [accountId, selectedAssetId]);

  // PRF-014 / PRF-080 — fetch on mount, on accountId change, and on asset-scope change.
  useEffect(() => {
    fetchPerformance();
  }, [fetchPerformance]);

  // PRF-080 / PRF-082 — load the account's holdings to populate the asset selector.
  useEffect(() => {
    let isMounted = true;
    setAssetOptions([]);
    (async () => {
      try {
        const result = await accountPerformanceGateway.getAccountHoldings(accountId);
        if (!isMounted) return;
        if (result.status === "ok") {
          setAssetOptions(presentAssetScopeOptions(result.data));
        } else {
          logger.error("[useAccountPerformance] holdings fetch failed", result.error);
          // F27 — surface the asset-selector load failure instead of a silently empty selector.
          const message = presentAccountPerformanceError(result.error);
          showSnackbar(t(message.key, message.vars), "error");
        }
      } catch (err) {
        if (!isMounted) return;
        logger.error("[useAccountPerformance] holdings fetch threw", { error: err });
        showSnackbar(t("account_performance.error.database_error"), "error");
      }
    })();
    return () => {
      isMounted = false;
    };
  }, [accountId, showSnackbar, t]);

  // PRF-060 — re-fetch on TransactionUpdated, AssetPriceUpdated, or AccountUpdated.
  useEffect(() => {
    const unlistenPromise = accountPerformanceGateway.subscribeToEvents((type) => {
      // MKT-181 — coalesce per-asset events while a bulk price fetch runs.
      if (type === "AssetPriceUpdated" && useAppStore.getState().priceFetch.active) {
        return;
      }
      if (
        type === "TransactionUpdated" ||
        type === "AssetPriceUpdated" ||
        type === "AssetPriceFetchCompleted" ||
        type === "AccountUpdated" ||
        type === "SyncCompleted"
      ) {
        fetchPerformance();
      }
    });
    return () => {
      void unlistenPromise?.then((unlisten) => unlisten());
    };
  }, [fetchPerformance]);

  // PRF-014 — remember the user's choice per account on every toggle.
  const selectViewMode = useCallback(
    (mode: PerformanceViewMode) => {
      setViewMode(mode);
      setPerfViewMode(accountId, mode);
    },
    [accountId],
  );

  const monthViewAvailable = data?.month_view_available ?? false;

  const isEmpty = useMemo(
    () => data !== null && data.yearly.length === 0 && data.monthly.length === 0,
    [data],
  );

  const availableYears = useMemo<number[]>(() => {
    if (!data) return [];
    const years = new Set<number>();
    for (const row of data.monthly) {
      years.add(row.year);
    }
    return [...years].sort((a, b) => b - a);
  }, [data]);

  // PRF-015 — the periods backing the active view: all yearly rows in year view,
  // the selected year's months in month view. Backend order (most-recent first).
  const activePeriods = useMemo(() => {
    if (!data) return [];
    if (viewMode === "year") {
      return data.yearly;
    }
    return data.monthly.filter((row) => selectedYear === null || row.year === selectedYear);
  }, [data, viewMode, selectedYear]);

  const rows = useMemo<PeriodRowViewModel[]>(
    () => activePeriods.map(presentPeriodRow),
    [activePeriods],
  );

  // Value-over-time series for the chart, chronological (oldest→newest) for the X axis.
  const chartPoints = useMemo<ValueChartPoint[]>(
    () => presentValueChartSeries(activePeriods),
    [activePeriods],
  );

  const selectedAssetName = useMemo(() => {
    if (selectedAssetId === null) return null;
    return assetOptions.find((option) => option.assetId === selectedAssetId)?.assetName ?? null;
  }, [assetOptions, selectedAssetId]);

  return {
    isLoading,
    error,
    retry: fetchPerformance,
    monthViewAvailable,
    isEmpty,
    viewMode,
    setViewMode: selectViewMode,
    availableYears,
    selectedYear,
    setSelectedYear,
    rows,
    chartPoints,
    assetOptions,
    selectedAssetId,
    setSelectedAssetId,
    selectedAssetName,
  };
}

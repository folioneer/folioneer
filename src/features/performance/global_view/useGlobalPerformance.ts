import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AccountPerformanceResponse } from "@/bindings";
import { logger } from "@/lib/logger";
import { getGlobalPerfViewMode, setGlobalPerfViewMode } from "@/lib/perfViewModeStorage";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { globalPerformanceGateway } from "../gateway";
import {
  type AccountScopeOption,
  presentAccountScopeOptions,
  presentAssetCatalogOptions,
} from "../shared/globalPresenter";
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

/** The read scope: both null = all accounts, per the GPF-010 matrix. */
interface PerformanceScope {
  accountId: string | null;
  assetId: string | null;
}

interface UseGlobalPerformanceResult {
  isLoading: boolean;
  error: I18nMessage | null;
  retry: () => void;
  monthViewAvailable: boolean;
  isEmpty: boolean;
  viewMode: PerformanceViewMode;
  setViewMode: (mode: PerformanceViewMode) => void;
  /** Years available in the monthly data, most-recent first. */
  availableYears: number[];
  selectedYear: number | null;
  setSelectedYear: (year: number) => void;
  /** Rows for the active view: yearly rows in year view, the selected year's months in month view. */
  rows: PeriodRowViewModel[];
  /** Value-over-time series for the line chart, chronological (oldest→newest). */
  chartPoints: ValueChartPoint[];
  /** Reporting currency of the response — EUR for cross-account scopes (GPF-011). */
  currency: string | null;
  /** Every account of the catalog, selectable as an account scope. */
  accountOptions: AccountScopeOption[];
  /** The scoped account, or null for all accounts. */
  selectedAccountId: string | null;
  setSelectedAccountId: (accountId: string | null) => void;
  /** Assets selectable in the current account scope: the account's non-cash holdings, or the catalog. */
  assetOptions: AssetScopeOption[];
  /** The scoped asset, or null for all assets. */
  selectedAssetId: string | null;
  setSelectedAssetId: (assetId: string | null) => void;
  /** "Account — Asset" scope suffix for the title; null when nothing is scoped (GPF-011). */
  scopeLabel: string | null;
}

export function useGlobalPerformance(): UseGlobalPerformanceResult {
  const accounts = useAppStore((state) => state.accounts);
  const catalogAssets = useAppStore((state) => state.assets);
  const [data, setData] = useState<AccountPerformanceResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [viewMode, setViewMode] = useState<PerformanceViewMode>("year");
  const [selectedYear, setSelectedYear] = useState<number | null>(null);
  const [scope, setScope] = useState<PerformanceScope>({ accountId: null, assetId: null });
  const [holdingOptions, setHoldingOptions] = useState<AssetScopeOption[]>([]);
  const showSnackbar = useSnackbar();
  const { t } = useTranslation();
  // Monotonic request token: only the latest fetchPerformance invocation may
  // commit its response, so an older in-flight read never clobbers a newer one.
  const requestSeqRef = useRef(0);

  // Changing the account scope resets the asset scope — the previous asset may
  // not exist in the new scope's selectable set.
  const setSelectedAccountId = useCallback((accountId: string | null) => {
    setScope({ accountId, assetId: null });
  }, []);

  const setSelectedAssetId = useCallback((assetId: string | null) => {
    setScope((previous) => ({ accountId: previous.accountId, assetId }));
  }, []);

  const fetchPerformance = useCallback(async () => {
    const requestSeq = ++requestSeqRef.current;
    setIsLoading(true);
    setError(null);
    try {
      const result = await globalPerformanceGateway.getGlobalPerformance(
        scope.accountId,
        scope.assetId,
      );
      // A newer fetch superseded this one while it was in flight — drop the response.
      if (requestSeq !== requestSeqRef.current) return;
      if (result.status === "ok") {
        setData(result.data);
        // GPF-016 — restore the remembered view mode (clamped to availability),
        // falling back to the default when there is no stored preference.
        setViewMode(resolveViewMode(getGlobalPerfViewMode(), result.data.month_view_available));
        const firstMonthlyYear = result.data.monthly[0]?.year ?? null;
        setSelectedYear(firstMonthlyYear);
      } else {
        logger.error("[useGlobalPerformance] fetch failed", result.error);
        setError(presentAccountPerformanceError(result.error));
      }
    } catch (err) {
      if (requestSeq !== requestSeqRef.current) return;
      logger.error("[useGlobalPerformance] fetch threw", { error: err });
      setError({ key: "account_performance.error.database_error" });
    } finally {
      if (requestSeq === requestSeqRef.current) {
        setIsLoading(false);
      }
    }
  }, [scope.accountId, scope.assetId]);

  // Fetch on mount and on every scope change (GPF-010).
  useEffect(() => {
    fetchPerformance();
  }, [fetchPerformance]);

  // Account scope selected → its non-cash holdings back the asset selector.
  useEffect(() => {
    let isMounted = true;
    setHoldingOptions([]);
    if (scope.accountId === null) return;
    const accountId = scope.accountId;
    (async () => {
      try {
        const result = await globalPerformanceGateway.getAccountHoldings(accountId);
        if (!isMounted) return;
        if (result.status === "ok") {
          setHoldingOptions(presentAssetScopeOptions(result.data));
        } else {
          logger.error("[useGlobalPerformance] holdings fetch failed", result.error);
          // F27 — surface the asset-selector load failure instead of a silently empty selector.
          const message = presentAccountPerformanceError(result.error);
          showSnackbar(t(message.key, message.vars), "error");
        }
      } catch (err) {
        if (!isMounted) return;
        logger.error("[useGlobalPerformance] holdings fetch threw", { error: err });
        showSnackbar(t("account_performance.error.database_error"), "error");
      }
    })();
    return () => {
      isMounted = false;
    };
  }, [scope.accountId, showSnackbar, t]);

  // Re-fetch on data mutations, mirroring the per-account page (PRF-060 / MKT-181), and
  // once on SyncCompleted for everything a sync applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = globalPerformanceGateway.subscribeToEvents((type) => {
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

  // GPF-016 — remember the user's choice on every toggle, mirroring the per-account page.
  const selectViewMode = useCallback((mode: PerformanceViewMode) => {
    setViewMode(mode);
    setGlobalPerfViewMode(mode);
  }, []);

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

  const chartPoints = useMemo<ValueChartPoint[]>(
    () => presentValueChartSeries(activePeriods),
    [activePeriods],
  );

  const accountOptions = useMemo(() => presentAccountScopeOptions(accounts), [accounts]);

  const assetOptions = useMemo(
    () => (scope.accountId === null ? presentAssetCatalogOptions(catalogAssets) : holdingOptions),
    [scope.accountId, catalogAssets, holdingOptions],
  );

  const scopeLabel = useMemo(() => {
    const accountName =
      scope.accountId === null
        ? null
        : (accountOptions.find((option) => option.accountId === scope.accountId)?.accountName ??
          null);
    const assetName =
      scope.assetId === null
        ? null
        : (assetOptions.find((option) => option.assetId === scope.assetId)?.assetName ?? null);
    const parts = [accountName, assetName].filter((part) => part !== null);
    return parts.length === 0 ? null : parts.join(" — ");
  }, [scope.accountId, scope.assetId, accountOptions, assetOptions]);

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
    currency: data?.currency ?? null,
    accountOptions,
    selectedAccountId: scope.accountId,
    setSelectedAccountId,
    assetOptions,
    selectedAssetId: scope.assetId,
    setSelectedAssetId,
    scopeLabel,
  };
}

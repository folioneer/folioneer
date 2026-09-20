import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { HoldingDetail } from "@/bindings";
import { isCashAsset } from "@/lib/cashAsset";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";
import {
  getPerfPeriod,
  setPerfPeriod as persistPerfPeriod,
  type StoredPerfPeriod,
} from "@/lib/perfPeriodStorage";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { accountDetailsGateway, useCachedAssets } from "../gateway";
import {
  priceHistoryBackfillErrorToI18n,
  priceHistoryBackfillOutcomeToMessage,
  priceRefreshLockErrorToI18n,
  toPriceableAssets,
} from "../shared/presenter";
import type { HoldingNoteTarget, ModalTarget, SellTarget, SplitTarget } from "../shared/types";
import { useAccountDetails } from "./useAccountDetails";

/** Local calendar date as ISO `YYYY-MM-DD` — the as-of selector's "today" default. */
function todayIso(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * Orchestration hook for AccountDetailsView. Bundles the data hook
 * (`useAccountDetails`) with the modal-state machine (Buy / Sell / Price /
 * Price-history / Open-balance / Deposit / Withdrawal) so the view component
 * stays a thin renderer.
 *
 * Splitting state out of the .tsx avoids the temptation to test the view's
 * orchestration via DOM-level RTL setups; this hook can be unit-tested in
 * isolation.
 */
export function useAccountDetailsView(accountId: string) {
  const navigate = useNavigate();
  // As-of valuation date: "" = live view (today); a non-empty ISO date loads a
  // read-only reconstruction. `isAsOf` is true only for a non-today date — picking
  // today (or clearing) keeps the live, mutable view.
  const [asOfDate, setAsOfDate] = useState("");
  const isAsOf = asOfDate !== "" && asOfDate !== todayIso();
  const data = useAccountDetails(accountId, isAsOf ? asOfDate : "");
  const accounts = useAppStore((state) => state.accounts);
  const cachedAssets = useCachedAssets();
  const fetchAssets = useAppStore((state) => state.fetchAssets);
  const showSnackbar = useSnackbar();
  const { t, i18n } = useTranslation();
  const accountCurrency = accounts.find((a) => a.id === accountId)?.currency ?? "";
  // FEE-076 — gate for every % management-fee surface on this view.
  const managementFeesEnabled =
    accounts.find((a) => a.id === accountId)?.management_fees_enabled ?? false;

  // ACD-054 — performance-column period, remembered per account. The windowed
  // returns are a live-view metric, so the as-of view pins the column to the
  // since-start figure and the setter is inert while a past date is selected.
  const [perfPeriodState, setPerfPeriodState] = useState<StoredPerfPeriod>(
    () => getPerfPeriod(accountId) ?? "since_start",
  );

  // Restore the remembered period when switching to another account without a remount.
  useEffect(() => {
    setPerfPeriodState(getPerfPeriod(accountId) ?? "since_start");
  }, [accountId]);

  const setPerfPeriod = useCallback(
    (period: StoredPerfPeriod) => {
      if (isAsOf) return;
      setPerfPeriodState(period);
      persistPerfPeriod(accountId, period);
    },
    [accountId, isAsOf],
  );

  // ---------------------------------------------------------------------------
  // Modal targets / open flags
  // ---------------------------------------------------------------------------
  const [buyTarget, setBuyTarget] = useState<ModalTarget | null>(null);
  const [sellTarget, setSellTarget] = useState<SellTarget | null>(null);
  const [historyTarget, setHistoryTarget] = useState<HoldingDetail | null>(null);
  const [openBalanceOpen, setOpenBalanceOpen] = useState(false);
  const [depositOpen, setDepositOpen] = useState(false);
  const [withdrawalOpen, setWithdrawalOpen] = useState(false);
  const [dividendOpen, setDividendOpen] = useState(false);
  const [freeSharesOpen, setFreeSharesOpen] = useState(false);
  const [managementFeeOpen, setManagementFeeOpen] = useState(false);
  const [interestOpen, setInterestOpen] = useState(false);
  const [feeScheduleTarget, setFeeScheduleTarget] = useState<{
    assetId: string;
    assetName: string;
  } | null>(null);
  const [splitTarget, setSplitTarget] = useState<SplitTarget | null>(null);
  const [holdingNoteTarget, setHoldingNoteTarget] = useState<HoldingNoteTarget | null>(null);

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------
  // ACD-035/036 — open the Add Transaction modal in place via the shell-mounted
  // AddTransactionModalMount (URL-driven), rather than navigating to a page. No
  // cross-feature import: the FAB only mutates URL params. No-op in as-of (read-only).
  const handleAddTransaction = useCallback(() => {
    if (isAsOf) return;
    patchModalSearch(navigate, { modal: "add-transaction", prefillAccountId: accountId });
  }, [navigate, accountId, isAsOf]);

  const handleBuyOpen = useCallback(
    (target: ModalTarget) => {
      if (isAsOf) return;
      setBuyTarget(target);
    },
    [isAsOf],
  );
  const handleBuyClose = useCallback(() => setBuyTarget(null), []);
  const handleBuySuccess = useCallback(() => {
    setBuyTarget(null);
    data.retry();
  }, [data]);

  const handleSellOpen = useCallback(
    (target: SellTarget) => {
      if (isAsOf) return;
      setSellTarget(target);
    },
    [isAsOf],
  );
  const handleSellClose = useCallback(() => setSellTarget(null), []);
  const handleSellSuccess = useCallback(() => {
    setSellTarget(null);
    data.retry();
  }, [data]);

  // MKT-072 — open price history modal for a holding
  const handlePriceHistory = useCallback(
    (assetId: string) => {
      const holding = data.holdingDetails.find((h) => h.asset_id === assetId);
      if (holding) setHistoryTarget(holding);
    },
    [data.holdingDetails],
  );
  const handleHistoryClose = useCallback(() => setHistoryTarget(null), []);

  const handleOpenBalanceOpen = useCallback(() => {
    if (isAsOf) return;
    setOpenBalanceOpen(true);
  }, [isAsOf]);
  const handleOpenBalanceClose = useCallback(() => setOpenBalanceOpen(false), []);
  const handleOpenBalanceSuccess = useCallback(() => {
    setOpenBalanceOpen(false);
    data.retry();
  }, [data]);

  const handleDepositOpen = useCallback(() => {
    if (isAsOf) return;
    setDepositOpen(true);
  }, [isAsOf]);
  const handleDepositClose = useCallback(() => setDepositOpen(false), []);
  const handleDepositSuccess = useCallback(() => {
    setDepositOpen(false);
    data.retry();
  }, [data]);

  const handleWithdrawalOpen = useCallback(() => {
    if (isAsOf) return;
    setWithdrawalOpen(true);
  }, [isAsOf]);
  const handleWithdrawalClose = useCallback(() => setWithdrawalOpen(false), []);
  const handleWithdrawalSuccess = useCallback(() => {
    setWithdrawalOpen(false);
    data.retry();
  }, [data]);

  // DIV-012 — dividend modal state (entered from the header "Add" menu)
  const handleDividendOpen = useCallback(() => {
    if (isAsOf) return;
    setDividendOpen(true);
  }, [isAsOf]);
  const handleDividendClose = useCallback(() => setDividendOpen(false), []);
  const handleDividendSuccess = useCallback(() => {
    setDividendOpen(false);
    data.retry();
  }, [data]);
  // DIV-010 — "add another" refreshes the data but keeps the modal open.
  const handleDividendRecorded = useCallback(() => {
    data.retry();
  }, [data]);

  // FSD-010/012 — free-shares modal state (entered from the header "Record" menu).
  const handleFreeSharesOpen = useCallback(() => {
    if (isAsOf) return;
    setFreeSharesOpen(true);
  }, [isAsOf]);
  const handleFreeSharesClose = useCallback(() => setFreeSharesOpen(false), []);
  const handleFreeSharesSuccess = useCallback(() => {
    setFreeSharesOpen(false);
    data.retry();
  }, [data]);

  // FEE-010 — one-off management-fee modal state (entered from the header "Record" menu).
  const handleManagementFeeOpen = useCallback(() => {
    if (isAsOf) return;
    setManagementFeeOpen(true);
  }, [isAsOf]);
  const handleManagementFeeClose = useCallback(() => setManagementFeeOpen(false), []);
  const handleManagementFeeSuccess = useCallback(() => {
    setManagementFeeOpen(false);
    data.retry();
  }, [data]);

  // INT-010 — interest modal state (entered from the header "Record" menu).
  const handleInterestOpen = useCallback(() => {
    if (isAsOf) return;
    setInterestOpen(true);
  }, [isAsOf]);
  const handleInterestClose = useCallback(() => setInterestOpen(false), []);
  const handleInterestSuccess = useCallback(() => {
    setInterestOpen(false);
    data.retry();
  }, [data]);

  // FEE-011 — recurring fee-schedule modal, opened per holding from its row action.
  const handleFeeScheduleOpen = useCallback(
    (assetId: string, assetName: string) => {
      if (isAsOf) return;
      setFeeScheduleTarget({ assetId, assetName });
    },
    [isAsOf],
  );
  const handleFeeScheduleClose = useCallback(() => setFeeScheduleTarget(null), []);
  const handleFeeScheduleSuccess = useCallback(() => {
    setFeeScheduleTarget(null);
    data.retry();
  }, [data]);

  // SPL-061 — split modal, opened per holding from its row action. The holding
  // detail supplies the quantity/average/latest price the modal's preview and
  // price prefill consume (SPL-040).
  const handleSplitOpen = useCallback(
    (assetId: string) => {
      if (isAsOf) return;
      const holding = data.holdingDetails.find((h) => h.asset_id === assetId);
      if (!holding) return;
      setSplitTarget({
        assetId: holding.asset_id,
        assetName: holding.asset_name,
        holdingQuantityMicro: holding.quantity,
        averagePriceMicro: holding.average_price,
        currentPriceMicro: holding.current_price,
      });
    },
    [isAsOf, data.holdingDetails],
  );
  const handleSplitClose = useCallback(() => setSplitTarget(null), []);
  const handleSplitSuccess = useCallback(() => {
    setSplitTarget(null);
    data.retry();
  }, [data]);

  // HNO-042 — note modal, opened per holding from its row action. The holding
  // detail supplies the stored note (text + alarm) the modal prefills from
  // (HNO-020); no note yet → create mode.
  const handleHoldingNoteOpen = useCallback(
    (assetId: string) => {
      if (isAsOf) return;
      const holding = data.holdingDetails.find((h) => h.asset_id === assetId);
      if (!holding) return;
      setHoldingNoteTarget({
        assetId: holding.asset_id,
        assetName: holding.asset_name,
        assetCurrency: holding.asset_currency,
        existing:
          holding.note_text !== null
            ? {
                text: holding.note_text,
                thresholdPrice: holding.note_threshold_price,
                thresholdDirection: holding.note_threshold_direction,
              }
            : null,
      });
    },
    [isAsOf, data.holdingDetails],
  );
  const handleHoldingNoteClose = useCallback(() => setHoldingNoteTarget(null), []);
  // HNO-022 — the success path re-fetches the read so the row's note + bell update.
  const handleHoldingNoteSuccess = useCallback(() => {
    setHoldingNoteTarget(null);
    data.retry();
  }, [data]);

  // MKT-153/156/157 — toggle the price-refresh lock on an asset. Calls the
  // block/unblock command, then re-reads the asset list (so the row's lock
  // icon flips from the store, mirroring archive/unarchive) and confirms
  // with a snackbar. Errors surface via the snackbar's i18n pipeline.
  const handleTogglePriceRefreshLock = useCallback(
    async (assetId: string, currentlyBlocked: boolean) => {
      if (isAsOf) return;
      try {
        const res = currentlyBlocked
          ? await accountDetailsGateway.unblockAssetPriceRefresh(assetId)
          : await accountDetailsGateway.blockAssetPriceRefresh(assetId);
        if (res.status === "ok") {
          await fetchAssets();
          showSnackbar(
            t(currentlyBlocked ? "mkt.lock.success_unblocked" : "mkt.lock.success_blocked"),
            "success",
          );
        } else {
          const msg = priceRefreshLockErrorToI18n(res.error);
          showSnackbar(t(msg.key, msg.vars), "error");
        }
      } catch (e) {
        logger.error("Failed to toggle price-refresh lock", { error: e, assetId });
        showSnackbar(t("error.Unknown"), "error");
      }
    },
    [fetchAssets, showSnackbar, t, isAsOf],
  );

  // MKT-190/197 — fill a holding's price history over the held period, then report
  // the outcome. Several holdings may run at once; each row reads its own flag.
  // reviewer-frontend FP: no data.retry() — the backfill publishes AssetPriceUpdated when it writes, and useAccountDetails re-fetches on it (MKT-194, MKT-036) — see PR #145
  const [backfillingAssetIds, setBackfillingAssetIds] = useState<string[]>([]);
  const handleBackfillPriceHistory = useCallback(
    async (assetId: string) => {
      if (isAsOf) return;
      setBackfillingAssetIds((running) => [...running, assetId]);
      try {
        const res = await accountDetailsGateway.backfillHoldingPriceHistory(accountId, assetId);
        if (res.status === "ok") {
          const message = priceHistoryBackfillOutcomeToMessage(res.data);
          showSnackbar(t(message.key, message.vars), message.variant);
        } else {
          const msg = priceHistoryBackfillErrorToI18n(res.error);
          showSnackbar(t(msg.key, msg.vars), "error");
        }
      } catch (e) {
        logger.error("Failed to backfill price history", { error: e, assetId });
        showSnackbar(t("error.Unknown"), "error");
      } finally {
        setBackfillingAssetIds((running) => running.filter((id) => id !== assetId));
      }
    },
    [accountId, isAsOf, showSnackbar, t],
  );

  // ---------------------------------------------------------------------------
  // Derived flags
  // ---------------------------------------------------------------------------
  // CSH-098 — the asset-positions empty state excludes the always-present Cash row.
  const hasNonCashActiveHoldings = data.holdings.some((row) => !row.isCash);
  const hasClosedHoldings = data.summary?.hasClosedHoldings ?? false;
  // Active, non-cash holdings (quantity > 0) — the candidate assets for the
  // dividend, free-shares, and management-fee modals (DIV-011/020, FSD-011, FEE-011/012).
  // Memoized so the stable reference does not invalidate each modal's `assetOptions`
  // memo on every parent render.
  const activeNonCashHoldings = useMemo(
    () =>
      data.holdingDetails
        .filter((h) => !isCashAsset(h.asset_id) && h.quantity > 0)
        .map((h) => ({
          assetId: h.asset_id,
          assetName: h.asset_name,
          assetCurrency: h.asset_currency,
        })),
    [data.holdingDetails],
  );
  // INT-020/023 — candidate assets for the interest modal: the account's cash
  // line (always a valid interest target, even at a zero balance) plus the
  // active non-cash holdings whose asset carries the `interest_bearing` flag
  // (AST-024). The holding detail does not carry the flag, so it is resolved
  // from the cached asset catalog. Memoized for the same stable-reference
  // reason as above.
  const interestEligibleHoldings = useMemo(() => {
    const interestBearingByAssetId = new Map(
      cachedAssets.map((asset) => [asset.id, asset.interest_bearing]),
    );
    return data.holdingDetails
      .filter(
        (h) =>
          isCashAsset(h.asset_id) ||
          (h.quantity > 0 && interestBearingByAssetId.get(h.asset_id) === true),
      )
      .map((h) => ({
        assetId: h.asset_id,
        assetName: h.asset_name,
        assetCurrency: h.asset_currency,
      }));
  }, [data.holdingDetails, cachedAssets]);
  // MKT-011 — priceable holdings for the price modal's asset combobox. Memoized
  // so the stable reference does not thrash the combobox on every parent render
  // (e.g. an AssetPriceUpdated event while the modal is open).
  const priceableAssets = useMemo(() => toPriceableAssets(data.holdings), [data.holdings]);
  return {
    // Data layer (re-exposed)
    isLoading: data.isLoading,
    error: data.error,
    retry: data.retry,
    summary: data.summary,
    holdings: data.holdings,
    holdingDetails: data.holdingDetails,
    closedHoldings: data.closedHoldings,
    // As-of (read-only past-date valuation)
    asOfDate,
    // As-of date formatted in the user's locale, for the read-only banner (F5).
    asOfDateFormatted: formatIsoDateNumeric(asOfDate, i18n.language),
    // Date shown in the selector: the chosen past date, or "" in the live view so
    // the field renders its "Today" placeholder instead of the literal current date.
    asOfDisplayDate: isAsOf ? asOfDate : "",
    setAsOfDate,
    isAsOf,
    // ACD-054 — selected performance-column period; since-start in the as-of view.
    perfPeriod: isAsOf ? ("since_start" as const) : perfPeriodState,
    setPerfPeriod,
    // Derived
    accountCurrency,
    managementFeesEnabled,
    hasNonCashActiveHoldings,
    hasClosedHoldings,
    activeNonCashHoldings,
    interestEligibleHoldings,
    priceableAssets,
    // Modal targets / flags
    buyTarget,
    sellTarget,
    historyTarget,
    openBalanceOpen,
    depositOpen,
    withdrawalOpen,
    dividendOpen,
    freeSharesOpen,
    managementFeeOpen,
    interestOpen,
    feeScheduleTarget,
    splitTarget,
    holdingNoteTarget,
    // Handlers
    handleAddTransaction,
    handleBuyOpen,
    handleBuyClose,
    handleBuySuccess,
    handleSellOpen,
    handleSellClose,
    handleSellSuccess,
    handlePriceHistory,
    handleHistoryClose,
    handleOpenBalanceOpen,
    handleOpenBalanceClose,
    handleOpenBalanceSuccess,
    handleDepositOpen,
    handleDepositClose,
    handleDepositSuccess,
    handleWithdrawalOpen,
    handleWithdrawalClose,
    handleWithdrawalSuccess,
    handleDividendOpen,
    handleDividendClose,
    handleDividendSuccess,
    handleDividendRecorded,
    handleFreeSharesOpen,
    handleFreeSharesClose,
    handleFreeSharesSuccess,
    handleManagementFeeOpen,
    handleManagementFeeClose,
    handleManagementFeeSuccess,
    handleInterestOpen,
    handleInterestClose,
    handleInterestSuccess,
    handleFeeScheduleOpen,
    handleFeeScheduleClose,
    handleFeeScheduleSuccess,
    handleSplitOpen,
    handleSplitClose,
    handleSplitSuccess,
    handleHoldingNoteOpen,
    handleHoldingNoteClose,
    handleHoldingNoteSuccess,
    handleTogglePriceRefreshLock,
    backfillingAssetIds,
    handleBackfillPriceHistory,
  };
}

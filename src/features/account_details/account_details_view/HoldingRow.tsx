import { useNavigate } from "@tanstack/react-router";
import {
  AlertTriangle,
  ArrowDownToLine,
  ArrowRightLeft,
  ArrowUpFromLine,
  Bell,
  BellRing,
  CalendarSync,
  Hand,
  History,
  Minus,
  PackageMinus,
  Percent,
  Plus,
  RefreshCw,
  Scissors,
  ScrollText,
  StickyNote,
} from "lucide-react";
import { type KeyboardEvent, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { isPositiveQuantity } from "@/lib/microUnits";
import { patchModalSearch } from "@/lib/modalSearch";
import type { StoredPerfPeriod } from "@/lib/perfPeriodStorage";
import { otherAccounts } from "@/lib/store";
import { IconButton } from "@/ui/components/button/IconButton";
import { formatIsoDateMedium } from "@/ui/format/date";
import { useCachedAccounts, useCachedAssets } from "../gateway";
import { PnlCell } from "../shared/PnlCell";
import { PINNED_ACTIONS_CELL, PINNED_ASSET_CELL } from "../shared/pinnedColumns";
import {
  type HoldingRowViewModel,
  hasNoPeriodFigure,
  selectPerformanceCell,
} from "../shared/presenter";
import type {
  ModalTarget,
  SellTarget,
  SplitTarget,
  TransferTarget,
  WriteOffTarget,
} from "../shared/types";

type HoldingRowProps = {
  row: HoldingRowViewModel;
  accountId: string;
  onBuy: (target: ModalTarget) => void;
  onSell: (target: SellTarget) => void;
  /** WOF-050 — open the write-off form for this holding. */
  onWriteOff?: (target: WriteOffTarget) => void;
  /** TRF-050 — open the transfer form for this holding. */
  onTransfer?: (target: TransferTarget) => void;
  onPriceHistory: (assetId: string) => void;
  /** Cash variant — open Deposit modal (CSH-091). */
  onDeposit?: () => void;
  /** Cash variant — open Withdrawal modal (CSH-091). */
  onWithdraw?: () => void;
  /** MKT-153/156 — toggle whether the asset's price is typed by hand (`price_refresh_blocked`). */
  onTogglePriceRefreshBlocked?: (assetId: string, currentlyBlocked: boolean) => void;
  /** MKT-190 — fill the asset's price history over the period the account held it. */
  onBackfillPriceHistory?: (assetId: string) => void;
  /** MKT-190 — true while this holding's backfill runs. */
  isBackfillingPriceHistory?: boolean;
  /** FEE-011 — open the recurring fee-schedule modal for this holding. */
  onManageFee?: (assetId: string, assetName: string) => void;
  /** SPL-061 — open the stock-split modal for this holding; MKT-233 — filled with an
   *  unrecorded split's date and ratio when one is given. */
  onSplit?: (assetId: string, prefill?: SplitTarget["prefill"]) => void;
  /** HNO-042 — open the holding-note modal for this holding. */
  onNote?: (assetId: string) => void;
  /** FEE-076 — render the Management Fees column; false when the account has the mechanism disabled. */
  showManagementFees?: boolean;
  /** ACD-054 — selected period for the Performance % column. */
  perfPeriod?: StoredPerfPeriod;
  /** As-of (read-only past-date view): hide every mutating action button. */
  readOnly?: boolean;
};

export function HoldingRow({
  row,
  accountId,
  onBuy,
  onSell,
  onWriteOff,
  onTransfer,
  onPriceHistory,
  onDeposit,
  onWithdraw,
  onTogglePriceRefreshBlocked,
  onBackfillPriceHistory,
  isBackfillingPriceHistory = false,
  onManageFee,
  onSplit,
  onNote,
  showManagementFees = true,
  perfPeriod = "since_start",
  readOnly = false,
}: HoldingRowProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const assets = useCachedAssets();
  const accounts = useCachedAccounts();

  const buildTarget = useCallback((): ModalTarget => {
    const asset = assets.find((a) => a.id === row.assetId);
    const account = accounts.find((a) => a.id === accountId);
    return {
      accountName: account?.name ?? accountId,
      assetId: row.assetId,
      assetName: row.assetName,
      assetCurrency: asset?.currency ?? "",
      showExchangeRate: asset && account ? asset.currency !== account.currency : false,
    };
  }, [assets, accounts, accountId, row]);

  const handleBuy = useCallback(() => {
    onBuy(buildTarget());
  }, [onBuy, buildTarget]);

  const handleSell = useCallback(() => {
    onSell({
      ...buildTarget(),
      assetReference: row.assetReference,
      holdingQuantity: row.heldQuantity,
    });
  }, [onSell, buildTarget, row.assetReference, row.heldQuantity]);

  const handleWriteOff = useCallback(() => {
    onWriteOff?.({
      assetId: row.assetId,
      assetName: row.assetName,
      assetReference: row.assetReference,
      heldQuantity: row.heldQuantity,
      averagePriceMicro: row.averagePriceMicro,
    });
  }, [onWriteOff, row]);

  // TRF-050 — the transfer form needs the sending account's name and currency.
  const sendingAccount = accounts.find((a) => a.id === accountId);
  const hasOtherAccount = otherAccounts(accounts, accountId).length > 0;
  const handleTransfer = useCallback(() => {
    onTransfer?.({
      assetId: row.assetId,
      assetName: row.assetName,
      assetReference: row.assetReference,
      heldQuantity: row.heldQuantity,
      averagePriceMicro: row.averagePriceMicro,
      accountName: sendingAccount?.name ?? "",
      accountCurrency: sendingAccount?.currency ?? "",
    });
  }, [onTransfer, row, sendingAccount]);

  const handleViewTransactions = useCallback(() => {
    navigate({
      to: "/accounts/$accountId/transactions/$assetId",
      params: { accountId, assetId: row.assetId },
      search: { pendingTransactionAssetId: undefined },
    });
  }, [navigate, accountId, row.assetId]);

  const handleEditMissingTicker = useCallback(() => {
    patchModalSearch(navigate, {
      modal: "edit-asset",
      editAssetId: row.assetId,
      focusField: "reference",
    });
  }, [navigate, row.assetId]);

  // FXR-012 — open the Record-FX-rate modal pre-filled with the foreign asset's
  // currency → the account currency, via the shell URL-modal mount (no cross-
  // feature import). `fxFrom` is the asset currency; `fxTo` the account currency.
  const handleRecordFxRate = useCallback(() => {
    const account = accounts.find((a) => a.id === accountId);
    patchModalSearch(navigate, {
      modal: "record-fx-rate",
      fxFrom: row.assetCurrency,
      fxTo: account?.currency ?? "",
    });
  }, [navigate, accounts, accountId, row.assetCurrency]);

  const handlePriceHistory = useCallback(() => {
    onPriceHistory(row.assetId);
  }, [onPriceHistory, row.assetId]);

  const handleBackfillPriceHistory = useCallback(() => {
    onBackfillPriceHistory?.(row.assetId);
  }, [onBackfillPriceHistory, row.assetId]);

  const handleManageFee = useCallback(() => {
    onManageFee?.(row.assetId, row.assetName);
  }, [onManageFee, row.assetId, row.assetName]);

  const handleSplit = useCallback(() => {
    onSplit?.(row.assetId);
  }, [onSplit, row.assetId]);

  const handleNote = useCallback(() => {
    onNote?.(row.assetId);
  }, [onNote, row.assetId]);

  const asset = assets.find((a) => a.id === row.assetId);
  const isArchived = asset?.is_archived ?? false;
  const isPriceRefreshBlocked = asset?.price_refresh_blocked ?? false;

  const handleTogglePriceRefreshBlocked = useCallback(() => {
    onTogglePriceRefreshBlocked?.(row.assetId, isPriceRefreshBlocked);
  }, [onTogglePriceRefreshBlocked, row.assetId, isPriceRefreshBlocked]);

  // Double-click a holding row to open the (router-driven) Edit Asset modal;
  // archived assets are not editable, mirroring the disabled edit affordance.
  const handleOpenAssetDetail = useCallback(() => {
    if (isArchived) return;
    patchModalSearch(navigate, { modal: "edit-asset", editAssetId: row.assetId });
  }, [navigate, row.assetId, isArchived]);

  // Keyboard parity with the double-click affordance: Enter on the focused row
  // opens the Edit Asset modal. Enter bubbling up from an inner interactive
  // element (action buttons, links) is never treated as a row action.
  const handleRowKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTableRowElement>) => {
      if (event.key !== "Enter" || event.defaultPrevented) return;
      if ((event.target as HTMLElement).closest("button, a, input, select, textarea")) return;
      event.preventDefault();
      handleOpenAssetDetail();
    },
    [handleOpenAssetDetail],
  );

  // CSH-091 — cash row variant: no Buy/Sell/Inspect, only Deposit/Withdraw.
  if (row.isCash) {
    return (
      <tr className="group m3-tr">
        {/* TODO-005 — actions first, filling two rows */}
        <td className={`m3-td ${PINNED_ACTIONS_CELL}`}>
          <div className="grid grid-flow-col grid-rows-2 gap-1 justify-start">
            {/* As-of view is read-only: Deposit/Withdraw are hidden (CSH-091). */}
            {!readOnly && (
              <>
                <IconButton
                  icon={<ArrowDownToLine size={16} />}
                  variant="success"
                  size="sm"
                  id={`action-record-deposit-${row.assetId}`}
                  aria-label={t("cash.action_record_deposit")}
                  onClick={onDeposit}
                />
                <IconButton
                  icon={<ArrowUpFromLine size={16} />}
                  variant="error"
                  size="sm"
                  id={`action-record-withdrawal-${row.assetId}`}
                  aria-label={t("cash.action_record_withdrawal")}
                  onClick={onWithdraw}
                  disabled={!isPositiveQuantity(row.heldQuantity)}
                />
              </>
            )}
            {/* CSH-110 — view the cash transaction history (deposits/withdrawals) */}
            <IconButton
              icon={<ScrollText size={16} />}
              size="sm"
              id={`action-view-transactions-${row.assetId}`}
              aria-label={t("transaction.list_title")}
              onClick={handleViewTransactions}
            />
          </div>
        </td>
        <td className={`m3-td ${PINNED_ASSET_CELL}`}>
          <div className="flex flex-col">
            <span className="font-medium text-m3-on-surface">{row.assetName}</span>
            <span className="text-xs text-m3-on-surface-variant">{row.assetReference}</span>
          </div>
        </td>
        <td className="m3-td text-right tabular-nums font-medium">{row.quantity}</td>
        <td className="m3-td" />
        <td className="m3-td" />
        <td className="m3-td" />
        <td className="m3-td" />
        {/* ACD-052 — cash weight % of the account's Global Value */}
        <td id={`holding-weight-pct-${row.assetId}`} className="m3-td text-right tabular-nums">
          {row.weightPct}
        </td>
        <td className="m3-td" />
        <td className="m3-td" />
        {/* DIV-072 — dividends / total-return columns are blank for the cash row */}
        <td className="m3-td" />
        <td className="m3-td" />
        {/* FEE-052 — management fees column is blank for the cash row (FEE-076: absent when disabled) */}
        {showManagementFees && <td className="m3-td" />}
      </tr>
    );
  }

  // ACD-054 — the Performance % cell for the selected period; since-start keeps
  // the existing figure, the windowed periods read their Simple Dietz return.
  const performanceCell = selectPerformanceCell(row, perfPeriod);

  return (
    <tr
      className="group m3-tr"
      id={`holding-row-${row.assetId}`}
      // Archived rows can't be edited (handleOpenAssetDetail no-ops), so they stay
      // out of the tab order rather than becoming an unlabeled keyboard dead-end.
      tabIndex={readOnly || isArchived ? undefined : 0}
      aria-label={
        readOnly || isArchived
          ? undefined
          : t("account_details.open_edit_asset", { name: row.assetName })
      }
      onDoubleClick={readOnly ? undefined : handleOpenAssetDetail}
      onKeyDown={readOnly ? undefined : handleRowKeyDown}
    >
      {/* TODO-005 — actions first, filling two rows */}
      <td className={`m3-td ${PINNED_ACTIONS_CELL}`}>
        <div className="grid grid-flow-col grid-rows-2 gap-1 justify-start">
          {/* As-of view is read-only: Buy/Sell/price-history/backfill/typed-by-hand toggle are hidden. */}
          {!readOnly && (
            <>
              {/* TRX-041 — Buy modal from holding row */}
              <IconButton
                icon={<Plus size={16} />}
                variant="success"
                size="sm"
                id={`action-buy-${row.assetId}`}
                aria-label={t("transaction.action_buy")}
                onClick={handleBuy}
              />
              {/* SEL-010 — Sell button; disabled when asset is archived (SEL-037) */}
              <IconButton
                icon={<Minus size={16} />}
                variant="error"
                size="sm"
                id={`action-sell-${row.assetId}`}
                aria-label={t("transaction.action_sell")}
                onClick={handleSell}
                disabled={isArchived}
              />
              {/* WOF-050 — Write-off action, after Sell (non-cash active rows; hidden for archived assets) */}
              {onWriteOff && !isArchived && (
                <IconButton
                  icon={<PackageMinus size={16} />}
                  size="sm"
                  id={`action-write-off-${row.assetId}`}
                  aria-label={t("transaction.action_write_off")}
                  onClick={handleWriteOff}
                />
              )}
              {/* TRF-050 — Transfer action, after Write off, while another account exists */}
              {onTransfer && !isArchived && hasOtherAccount && (
                <IconButton
                  icon={<ArrowRightLeft size={16} />}
                  size="sm"
                  id={`action-transfer-${row.assetId}`}
                  aria-label={t("transaction.action_transfer")}
                  onClick={handleTransfer}
                />
              )}
              {/* SPL-061 — Split action (non-cash active rows; hidden for archived assets) */}
              {onSplit && !isArchived && (
                <IconButton
                  icon={<Scissors size={16} />}
                  size="sm"
                  id={`action-split-${row.assetId}`}
                  aria-label={t("transaction.action_split")}
                  onClick={handleSplit}
                />
              )}
              {/* HNO-042 — Note action (non-cash active rows; hidden for archived assets) */}
              {onNote && !isArchived && (
                <IconButton
                  icon={<StickyNote size={16} />}
                  size="sm"
                  id={`action-note-${row.assetId}`}
                  aria-label={t("account_details.action_note")}
                  onClick={handleNote}
                />
              )}
              {/* MKT-070 — Price history button (active holdings only); add-price lives inside */}
              {row.canEnterPrice && (
                <IconButton
                  icon={<History size={16} />}
                  size="sm"
                  id={`action-price-history-${row.assetId}`}
                  aria-label={t("account_details.action_price_history")}
                  onClick={handlePriceHistory}
                />
              )}
              {/* MKT-190 — fill the price history of the period the account held the asset */}
              {onBackfillPriceHistory && (
                <IconButton
                  icon={<CalendarSync size={16} />}
                  size="sm"
                  id={`action-backfill-price-history-${row.assetId}`}
                  aria-label={t("mkt.backfill.action")}
                  onClick={handleBackfillPriceHistory}
                  disabled={isBackfillingPriceHistory}
                  className={isBackfillingPriceHistory ? "animate-pulse" : ""}
                />
              )}
              {/* MKT-153 — typed by hand, or fetched automatically (ADR-014); the icon is
                  the state, the name the action */}
              {onTogglePriceRefreshBlocked && (
                <IconButton
                  icon={isPriceRefreshBlocked ? <Hand size={16} /> : <RefreshCw size={16} />}
                  size="sm"
                  id={`action-toggle-price-refresh-${row.assetId}`}
                  aria-label={t(
                    isPriceRefreshBlocked
                      ? "mkt.typed_by_hand.action_fetch_automatically"
                      : "mkt.typed_by_hand.action_type_by_hand",
                  )}
                  onClick={handleTogglePriceRefreshBlocked}
                />
              )}
              {/* FEE-011 — manage the recurring management-fee schedule for this holding */}
              {onManageFee && (
                <IconButton
                  icon={<Percent size={16} />}
                  size="sm"
                  id={`action-manage-fee-${row.assetId}`}
                  aria-label={t("account_details.action_manage_fee")}
                  onClick={handleManageFee}
                />
              )}
            </>
          )}
          <IconButton
            icon={<ScrollText size={16} />}
            size="sm"
            id={`action-view-transactions-${row.assetId}`}
            aria-label={t("transaction.list_title")}
            onClick={handleViewTransactions}
          />
        </div>
      </td>
      <td className={`m3-td ${PINNED_ASSET_CELL}`}>
        <div className="flex flex-col">
          <span className="font-medium text-m3-on-surface">{row.assetName}</span>
          <span className="text-xs text-m3-on-surface-variant">{row.assetReference}</span>
          {/* SYN-040 — derived after a merge; clears by itself once the ledger is valid again */}
          {row.inconsistencyLabel !== null && (
            <span
              id={`holding-inconsistency-${row.assetId}`}
              className="flex items-center gap-1 text-xs text-m3-error"
            >
              <AlertTriangle size={12} className="shrink-0" aria-hidden="true" />
              {t(row.inconsistencyLabel.key, row.inconsistencyLabel.vars)}
            </span>
          )}
          {/* MKT-233 — each split the provider reports that the account has not recorded;
              MKT-234 — with a shortcut to record it; nothing is recorded automatically. */}
          {row.unrecordedSplits.map((split) => (
            <span
              key={split.date}
              id={`holding-unrecorded-split-${row.assetId}-${split.date}`}
              className="flex items-center gap-1 text-xs text-m3-warning"
            >
              <AlertTriangle size={12} className="shrink-0" aria-hidden="true" />
              {t("mkt.unrecorded_split.row_label", {
                new: split.new,
                old: split.old,
                date: formatIsoDateMedium(split.date, i18n.language),
              })}
              {!readOnly && onSplit && !isArchived && (
                <button
                  type="button"
                  id={`action-record-unrecorded-split-${row.assetId}-${split.date}`}
                  onClick={() =>
                    onSplit(row.assetId, { date: split.date, new: split.new, old: split.old })
                  }
                  className="ml-1 text-m3-primary underline-offset-2 hover:underline focus:outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-m3-primary"
                >
                  {t("mkt.unrecorded_split.action_record")}
                </button>
              )}
            </span>
          ))}
          {/* HNO-041 — pinned note: optional bell (armed / triggered) + single
              truncated line, full text as tooltip */}
          {row.noteText !== null && (
            <span
              id={`holding-note-${row.assetId}`}
              title={row.noteText}
              className="flex items-center gap-1 max-w-56 text-xs text-m3-on-surface-variant"
            >
              {row.noteHasAlarm &&
                (row.noteAlarmTriggered ? (
                  <BellRing
                    id={`holding-note-bell-${row.assetId}`}
                    size={12}
                    className="shrink-0 text-m3-error fill-current"
                    role="img"
                    aria-label={t("holding_note.bell_triggered")}
                  />
                ) : (
                  <Bell
                    id={`holding-note-bell-${row.assetId}`}
                    size={12}
                    className="shrink-0 text-m3-on-surface-variant"
                    role="img"
                    aria-label={t("holding_note.bell_armed")}
                  />
                ))}
              <span className="truncate">{row.noteText}</span>
            </span>
          )}
        </div>
      </td>
      <td id={`holding-quantity-${row.assetId}`} className="m3-td text-right tabular-nums">
        {row.quantity}
      </td>
      <td id={`holding-avg-price-${row.assetId}`} className="m3-td text-right tabular-nums">
        {row.averagePrice}
      </td>
      {/* SEL-042 — Realized P&L */}
      <td className="m3-td text-right">
        <PnlCell value={row.realizedPnl} raw={row.realizedPnlRaw} />
        {/* WOF-059 — the lost part of the realized P&L, when there is one */}
        {row.realizedLost !== null && (
          <div
            id={`holding-realized-lost-${row.assetId}`}
            className="text-xs text-m3-on-surface-variant"
          >
            {t("account_details.realized_lost", { amount: row.realizedLost })}
          </div>
        )}
      </td>
      {/* MKT-030 — Current price; MKT-032 diagnostic states; MKT-140 staleness; MKT-142 source badge */}
      <td id={`holding-current-price-${row.assetId}`} className="m3-td text-right tabular-nums">
        <div className="flex flex-col items-end gap-0.5">
          {row.currentPrice.kind === "present" ? (
            <>
              <span>{row.currentPrice.formatted}</span>
              <div className="flex items-center gap-1.5">
                {row.sourceLabel && (
                  <span className="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded bg-m3-surface-container-highest text-m3-on-surface-variant">
                    {t(row.sourceLabel)}
                  </span>
                )}
                {/* MKT-207 — an outdated price says so in place of its quiet age */}
                {row.staleness &&
                  (row.priceOutdated ? (
                    <span
                      id={`holding-price-outdated-${row.assetId}`}
                      className="flex items-center gap-1 whitespace-nowrap text-[10px] font-medium text-m3-warning"
                    >
                      <AlertTriangle size={11} aria-hidden="true" />
                      {t("mkt.outdated.row_label", {
                        age: t(row.staleness.key, row.staleness.params),
                      })}
                    </span>
                  ) : (
                    <span className="text-[10px] text-m3-on-surface-variant">
                      {t(row.staleness.key, row.staleness.params)}
                    </span>
                  ))}
              </div>
            </>
          ) : row.currentPrice.kind === "missing_ticker" ? (
            /* As-of view is read-only: the edit-missing-ticker shortcut (a write)
             is hidden; the plain state text shows instead. */
            !readOnly ? (
              <button
                type="button"
                id={`action-edit-missing-ticker-${row.assetId}`}
                onClick={handleEditMissingTicker}
                className="text-m3-primary text-sm underline-offset-2 hover:underline focus:outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-m3-primary"
              >
                {t("mkt.price_state.missing_ticker")}
              </button>
            ) : (
              <span className="text-m3-on-surface-variant text-sm">
                {t("mkt.price_state.missing_ticker")}
              </span>
            )
          ) : (
            <span className="text-m3-on-surface-variant text-sm">
              {t("mkt.price_state.no_price_available")}
            </span>
          )}
          {/* MKT-222 — why this device's last fetch did not update the price */}
          {row.fetchFailure && (
            <span
              id={`holding-price-fetch-failure-${row.assetId}`}
              title={
                row.fetchFailedOn
                  ? t("mkt.fetch_failure.tooltip", { date: row.fetchFailedOn })
                  : undefined
              }
              className="max-w-56 text-right text-[10px] text-m3-warning"
            >
              {t(row.fetchFailure.key)}
            </span>
          )}
          {/* MKT-158 — a price typed by hand says so, under the price or under the
            diagnostic of a missing one */}
          {isPriceRefreshBlocked && (
            <span
              id={`holding-price-typed-by-hand-${row.assetId}`}
              className="flex items-center gap-1 text-[10px] text-m3-on-surface-variant"
            >
              <Hand size={11} aria-hidden="true" />
              {t("mkt.typed_by_hand.row_label")}
            </span>
          )}
        </div>
      </td>
      {/* MKT-143 — Current value = current price × quantity (asset currency) */}
      <td id={`holding-current-value-${row.assetId}`} className="m3-td text-right tabular-nums">
        {row.currentValue}
      </td>
      {/* ACD-052 — Weight % of the account's Global Value */}
      <td id={`holding-weight-pct-${row.assetId}`} className="m3-td text-right tabular-nums">
        {row.weightPct}
      </td>
      {/* MKT-032/034 — Unrealized P&L; FXR-012 — foreign-currency holdings with a
          price but no usable rate show a Record-FX-rate shortcut instead of "—" */}
      <td className="m3-td text-right">
        {row.unrealizedPnl !== "—" ? (
          <div className="flex flex-col items-end gap-0.5">
            <PnlCell value={row.unrealizedPnl} raw={row.unrealizedPnlRaw ?? 0} />
            {row.fxStaleness && (
              <span className="text-[10px] text-m3-on-surface-variant">
                {t(row.fxStaleness.key, row.fxStaleness.params)}
              </span>
            )}
          </div>
        ) : row.currentPrice.kind === "present" && !readOnly ? (
          /* As-of view is read-only: the Record-FX-rate shortcut (a write) is
             hidden. */
          <button
            type="button"
            data-testid={`action-record-fx-rate-${row.assetId}`}
            onClick={handleRecordFxRate}
            className="text-m3-primary text-sm underline-offset-2 hover:underline focus:outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-m3-primary"
          >
            {t("currency.action_record_fx_rate")}
          </button>
        ) : (
          <span className="text-m3-on-surface-variant">{row.unrealizedPnl}</span>
        )}
      </td>
      {/* MKT-035 / ACD-054 — Performance % over the selected period; ACD-057 — a window
          with no figure says why on hover */}
      <td
        id={`holding-period-performance-${row.assetId}`}
        title={
          hasNoPeriodFigure(row, perfPeriod) ? t("account_details.perf_period_none") : undefined
        }
        className="m3-td text-right tabular-nums"
      >
        {performanceCell.formatted !== "—" ? (
          <span
            className={
              performanceCell.raw !== null && performanceCell.raw < 0
                ? "text-m3-loss"
                : "text-m3-gain"
            }
          >
            {performanceCell.formatted}
          </span>
        ) : (
          <span className="text-m3-on-surface-variant">{performanceCell.formatted}</span>
        )}
      </td>
      {/* DIV-072 — Dividends received (always shown) */}
      <td
        id={`holding-dividends-received-${row.assetId}`}
        className="m3-td text-right tabular-nums"
      >
        {row.dividendsReceived}
      </td>
      {/* DIV-072 — Total return % (price + dividends); "—" when not computable */}
      <td className="m3-td text-right tabular-nums">
        {row.totalReturnPct !== "—" ? (
          <span
            className={
              row.totalReturnPctRaw !== null && row.totalReturnPctRaw < 0
                ? "text-m3-loss"
                : "text-m3-gain"
            }
          >
            {row.totalReturnPct}
          </span>
        ) : (
          <span className="text-m3-on-surface-variant">{row.totalReturnPct}</span>
        )}
      </td>
      {/* FEE-052 — Management fees deducted; FEE-074 — the active schedule's annual
          rate rides along when one exists; FEE-076 — column absent when the account
          has the mechanism disabled */}
      {showManagementFees && (
        <td id={`holding-management-fees-${row.assetId}`} className="m3-td text-right tabular-nums">
          {row.feeRatePct !== null ? (
            <>
              {row.managementFees}{" "}
              <span
                id={`holding-fee-rate-${row.assetId}`}
                className="text-m3-on-surface-variant text-xs"
              >
                · {row.feeRatePct}
              </span>
            </>
          ) : (
            row.managementFees
          )}
        </td>
      )}
    </tr>
  );
}

import { useNavigate } from "@tanstack/react-router";
import { CalendarSync, ScrollText } from "lucide-react";
import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "@/ui/components/button/IconButton";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { PnlCell } from "../shared/PnlCell";
import { PINNED_ACTIONS_CELL, PINNED_ASSET_CELL } from "../shared/pinnedColumns";
import type { ClosedHoldingRowViewModel } from "../shared/presenter";

type ClosedHoldingRowProps = {
  row: ClosedHoldingRowViewModel;
  accountId: string;
  /** MKT-190 — fill the price history of the held period; absent in the as-of view. */
  onBackfillPriceHistory?: (assetId: string) => void;
  /** MKT-190 — true while this holding's backfill runs. */
  isBackfillingPriceHistory?: boolean;
};

export function ClosedHoldingRow({
  row,
  accountId,
  onBackfillPriceHistory,
  isBackfillingPriceHistory = false,
}: ClosedHoldingRowProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();

  const handleViewTransactions = useCallback(() => {
    navigate({
      to: "/accounts/$accountId/transactions/$assetId",
      params: { accountId, assetId: row.assetId },
      search: { pendingTransactionAssetId: undefined },
    });
  }, [navigate, accountId, row.assetId]);

  const handleBackfillPriceHistory = useCallback(() => {
    onBackfillPriceHistory?.(row.assetId);
  }, [onBackfillPriceHistory, row.assetId]);

  // #019 — a closed line is set apart by muting its figures and its asset text. The
  // Actions cell stays at full strength so an enabled button looks enabled, and the pinned
  // Asset cell stays opaque — only its content is muted — so scrolled columns never show
  // through it.
  return (
    <tr className="group m3-tr">
      {/* ACD-049 / #005 — actions first; Buy/Sell omitted for closed positions */}
      <td className={`m3-td ${PINNED_ACTIONS_CELL}`}>
        <div className="grid grid-flow-col grid-rows-2 gap-1 justify-start">
          <IconButton
            icon={<ScrollText size={16} />}
            size="sm"
            id={`action-view-closed-transactions-${row.assetId}`}
            aria-label={t("transaction.list_title")}
            onClick={handleViewTransactions}
          />
          {/* MKT-190 — fill the price history of the period the position was held */}
          {onBackfillPriceHistory && (
            <IconButton
              icon={<CalendarSync size={16} />}
              size="sm"
              id={`action-backfill-closed-price-history-${row.assetId}`}
              aria-label={t("mkt.backfill.action")}
              onClick={handleBackfillPriceHistory}
              disabled={isBackfillingPriceHistory}
              className={isBackfillingPriceHistory ? "animate-pulse" : ""}
            />
          )}
        </div>
      </td>
      {/* #021 — as wide as its content; the spare width goes to the unpinned columns */}
      <td className={`m3-td ${PINNED_ASSET_CELL} w-px whitespace-nowrap`}>
        <div className="flex flex-col opacity-70">
          <span className="font-medium text-m3-on-surface">{row.assetName}</span>
          <span className="text-xs text-m3-on-surface-variant">{row.assetReference}</span>
        </div>
      </td>
      <td className="m3-td text-right opacity-70">
        <PnlCell value={row.realizedPnl} raw={row.realizedPnlRaw} />
      </td>
      {/* DIV-073 — dividends received over the position's life */}
      <td className="m3-td text-right text-m3-on-surface opacity-70">{row.dividendsReceived}</td>
      {/* Total revenues = realized P&L + dividends */}
      <td className="m3-td text-right opacity-70">
        <PnlCell value={row.totalRevenues} raw={row.totalRevenuesRaw} />
      </td>
      <td className="m3-td text-right text-m3-on-surface-variant opacity-70">
        {formatIsoDateNumeric(row.lastSoldDate, i18n.language)}
      </td>
    </tr>
  );
}

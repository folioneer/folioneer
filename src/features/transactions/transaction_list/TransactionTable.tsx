import { Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { IconButton } from "@/ui/components/button/IconButton";
import { SortIcon } from "@/ui/components/SortIcon";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { TransactionRowViewModel } from "../shared/presenter";

interface TransactionTableProps {
  /** Already-sorted rows to render. */
  rows: TransactionRowViewModel[];
  sortDirection: "asc" | "desc";
  onToggleSort: () => void;
  /** Show an Asset column — used by the account-wide journal where rows span assets. */
  showAssetColumn?: boolean;
  /**
   * Bank-statement mode: replace the single Total Amount column with Cash out /
   * Cash in / Balance (driven by each row's `cashOut`/`cashIn`/`balance`).
   */
  cashStatement?: boolean;
  onEditTransaction: (transactionId: string) => void;
  onDeleteTransaction: (transactionId: string) => void;
}

/**
 * Presentational transaction table shared by the per-asset journal and the
 * account-wide journal. Owns no data fetching or modal state — edit/delete
 * intents bubble out via callbacks keyed by transaction id.
 */
export function TransactionTable({
  rows,
  sortDirection,
  onToggleSort,
  showAssetColumn = false,
  cashStatement = false,
  onEditTransaction,
  onDeleteTransaction,
}: TransactionTableProps) {
  const { t, i18n } = useTranslation();

  return (
    <div className="m3-table-container flex-1">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 bg-m3-surface-container z-10">
          <tr>
            <th className="m3-th">{t("transaction.column_type")}</th>
            {showAssetColumn && <th className="m3-th">{t("transaction.column_asset")}</th>}
            <th className="m3-th">
              <button
                type="button"
                id="txl-sort-date"
                onClick={onToggleSort}
                className="flex items-center cursor-pointer hover:text-m3-primary transition-colors"
              >
                {t("transaction.column_date")}
                <SortIcon active direction={sortDirection} />
              </button>
            </th>
            <th className="m3-th text-right">{t("transaction.column_quantity")}</th>
            <th className="m3-th text-right">{t("transaction.column_unit_price")}</th>
            <th className="m3-th text-right">{t("transaction.column_exchange_rate")}</th>
            <th className="m3-th text-right">{t("transaction.column_fees")}</th>
            {cashStatement ? (
              <>
                <th className="m3-th text-right">{t("transaction.column_cash_out")}</th>
                <th className="m3-th text-right">{t("transaction.column_cash_in")}</th>
                <th className="m3-th text-right">{t("transaction.column_balance")}</th>
              </>
            ) : (
              <th className="m3-th text-right">{t("transaction.column_total_amount")}</th>
            )}
            <th className="m3-th text-right">{t("transaction.column_realized_pnl")}</th>
            <th className="m3-th">{t("transaction.column_actions")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            // FSD-050 / FEE-055 / SPL-060 / INT-030 — a free-share distribution, a
            // management-fee deduction, a stock split, or an interest credit moves no
            // money: the unit-price and total-amount columns render the neutral
            // placeholder (the quantity column still shows the credited/removed
            // units, or the ×N ratio label for a split).
            const isQuantityOnly =
              row.type === "FreeShares" ||
              row.type === "ManagementFee" ||
              row.type === "Split" ||
              row.type === "Interest";
            const moneyDash = (
              <span className="text-m3-on-surface-variant">
                {t("account_details.pnl_placeholder")}
              </span>
            );
            return (
              <tr key={row.id} id={`txl-row-${row.id}`} className="m3-tr">
                <td className="m3-td">{t(`transaction.type_${row.type.toLowerCase()}`)}</td>
                {showAssetColumn && (
                  <td id={`txl-asset-${row.id}`} className="m3-td">
                    {row.assetName}
                  </td>
                )}
                <td className="m3-td tabular-nums">
                  {formatIsoDateNumeric(row.date, i18n.language)}
                </td>
                <td id={`txl-qty-${row.id}`} className="m3-td text-right tabular-nums">
                  {row.quantity}
                </td>
                <td id={`txl-unit-price-${row.id}`} className="m3-td text-right tabular-nums">
                  {isQuantityOnly ? moneyDash : row.unitPrice}
                </td>
                <td className="m3-td text-right tabular-nums">{row.exchangeRate}</td>
                <td className="m3-td text-right tabular-nums">{row.fees}</td>
                {cashStatement ? (
                  <>
                    {/* cashOut/cashIn use `||` — an empty string means "no cash this
                        side", so it falls back to the placeholder; `balance` uses `??`
                        because "0" is a meaningful value that must still render. */}
                    <td
                      id={`txl-cash-out-${row.id}`}
                      className="m3-td text-right tabular-nums text-m3-debit"
                    >
                      {row.cashOut || moneyDash}
                    </td>
                    <td
                      id={`txl-cash-in-${row.id}`}
                      className="m3-td text-right tabular-nums text-m3-credit"
                    >
                      {row.cashIn || moneyDash}
                    </td>
                    <td
                      id={`txl-balance-${row.id}`}
                      className="m3-td text-right tabular-nums font-medium"
                    >
                      {row.balance ?? moneyDash}
                    </td>
                  </>
                ) : (
                  <td
                    id={`txl-total-${row.id}`}
                    className="m3-td text-right tabular-nums font-medium"
                  >
                    {isQuantityOnly ? moneyDash : row.totalAmount}
                  </td>
                )}
                {/* SEL-041 — Realized P&L column (SEL-043: zero/null shown as placeholder) */}
                <td className="m3-td text-right tabular-nums">
                  {row.realizedPnlRaw != null && row.realizedPnlRaw !== 0 ? (
                    <span className={row.realizedPnlRaw > 0 ? "text-m3-gain" : "text-m3-loss"}>
                      {row.realizedPnl}
                    </span>
                  ) : (
                    <span className="text-m3-on-surface-variant">
                      {t("account_details.pnl_placeholder")}
                    </span>
                  )}
                </td>
                <td className="m3-td">
                  <div className="flex items-center gap-1">
                    <IconButton
                      icon={<Pencil size={16} />}
                      size="sm"
                      id={`txl-edit-${row.id}`}
                      aria-label={t("action.edit")}
                      onClick={() => onEditTransaction(row.id)}
                    />
                    <IconButton
                      icon={<Trash2 size={16} />}
                      size="sm"
                      variant="danger"
                      id={`txl-delete-${row.id}`}
                      aria-label={t("action.delete")}
                      onClick={() => onDeleteTransaction(row.id)}
                    />
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

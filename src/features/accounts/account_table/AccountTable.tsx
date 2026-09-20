import { AlertTriangle, Calendar, ChevronRight, Edit2, Trash2, X } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { microToFormatted } from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { SortIcon } from "@/ui/components/SortIcon";
import { EditAccountModal } from "../edit_account_modal/EditAccountModal";
import {
  FREQUENCY_I18N_KEYS,
  formatAccountRowTotalUnrealizedPnl,
  formatAccountRowYtdPerformancePct,
} from "../shared/presenter";
import { useAccountSummaries } from "../useAccountSummaries";
import { useAccounts } from "../useAccounts";
import { useAccountTable } from "./useAccountTable";

interface AccountTableProps {
  searchTerm: string;
  /** ACD-010 — callback when the user clicks an account row (excluding action buttons). */
  onAccountClick: (accountId: string) => void;
}

export function AccountTable({ searchTerm, onAccountClick }: AccountTableProps) {
  const { t } = useTranslation();
  // ACC-021 — list-page data comes from get_account_summaries (enriched with
  // total_global_value); mutations (delete + delete-summary) remain on the bare
  // useAccounts hook since they don't need the per-account value.
  const {
    summaries,
    portfolioTotal,
    isLoading: loading,
    error: fetchError,
    refetch,
  } = useAccountSummaries();
  const { deleteAccount, getAccountDeletionSummary } = useAccounts();

  useEffect(() => {
    logger.info("[AccountTable] mounted");
  }, []);

  const {
    sortedAndFilteredAccounts,
    sortConfig,
    handleSort,
    handleNameKeyDown,
    handleBankNameKeyDown,
    handleFrequencyKeyDown,
    handleGlobalValueKeyDown,
    handleUnrealizedPnlKeyDown,
    handleYtdPctKeyDown,
    handleRowKeyDown,
    handleEditClick,
    handleEditClose,
    handleDeleteClick,
    handleDeleteCancel,
    isEmpty,
    hasNoSearchResults,
    deleteData,
    deleteSummary,
    fetchingSummaryFor,
    editData,
    actionError,
    setActionError,
    handleDeleteConfirm,
  } = useAccountTable(
    summaries,
    searchTerm,
    deleteAccount,
    getAccountDeletionSummary,
    onAccountClick,
  );

  // ACC-032 — the portfolio total shows only while every account is listed.
  const showPortfolioTotal =
    !loading &&
    !isEmpty &&
    !fetchError &&
    searchTerm.trim().length === 0 &&
    portfolioTotal !== null;

  return (
    <div className="m3-table-container flex-1">
      {/* R13 — inline action error with dismiss */}
      {actionError && (
        <div
          role="alert"
          className="mb-3 flex items-center justify-between gap-2 text-sm text-m3-error px-2"
        >
          <span>{t(actionError.key, actionError.vars)}</span>
          <IconButton
            icon={<X size={14} />}
            size="sm"
            aria-label={t("action.close")}
            onClick={() => setActionError(null)}
          />
        </div>
      )}
      <table className="w-full border-collapse">
        <thead className="sticky top-0 bg-m3-surface-container z-10">
          <tr>
            <th
              className="m3-th cursor-pointer"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "name"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("name")}
              onKeyDown={handleNameKeyDown}
            >
              <div className="flex items-center">
                {t("account.column_name")}
                <SortIcon
                  active={sortConfig.key === "name"}
                  direction={sortConfig.key === "name" ? sortConfig.direction : null}
                />
              </div>
            </th>
            <th
              id="account-column-bank"
              className="m3-th cursor-pointer"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "bank_name"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("bank_name")}
              onKeyDown={handleBankNameKeyDown}
            >
              <div className="flex items-center">
                {t("account.column_bank_name")}
                <SortIcon
                  active={sortConfig.key === "bank_name"}
                  direction={sortConfig.key === "bank_name" ? sortConfig.direction : null}
                />
              </div>
            </th>
            <th
              className="m3-th cursor-pointer"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "update_frequency"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("update_frequency")}
              onKeyDown={handleFrequencyKeyDown}
            >
              <div className="flex items-center">
                {t("account.column_frequency")}
                <SortIcon
                  active={sortConfig.key === "update_frequency"}
                  direction={sortConfig.key === "update_frequency" ? sortConfig.direction : null}
                />
              </div>
            </th>
            <th
              id="account-column-global-value"
              className="m3-th cursor-pointer text-right"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "total_global_value"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("total_global_value")}
              onKeyDown={handleGlobalValueKeyDown}
            >
              <div className="flex items-center justify-end">
                {t("account.column_global_value")}
                <SortIcon
                  active={sortConfig.key === "total_global_value"}
                  direction={sortConfig.key === "total_global_value" ? sortConfig.direction : null}
                />
              </div>
            </th>
            <th
              id="account-column-unrealized-pnl"
              className="m3-th cursor-pointer text-right"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "total_unrealized_pnl"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("total_unrealized_pnl")}
              onKeyDown={handleUnrealizedPnlKeyDown}
            >
              <div className="flex items-center justify-end">
                {t("account.column_unrealized_pnl")}
                <SortIcon
                  active={sortConfig.key === "total_unrealized_pnl"}
                  direction={
                    sortConfig.key === "total_unrealized_pnl" ? sortConfig.direction : null
                  }
                />
              </div>
            </th>
            <th
              id="account-column-ytd-pct"
              className="m3-th cursor-pointer text-right"
              tabIndex={0}
              scope="col"
              aria-sort={
                sortConfig.key === "ytd_performance_pct"
                  ? sortConfig.direction === "asc"
                    ? "ascending"
                    : "descending"
                  : "none"
              }
              onClick={() => handleSort("ytd_performance_pct")}
              onKeyDown={handleYtdPctKeyDown}
            >
              <div className="flex items-center justify-end">
                {t("account.column_ytd_performance")}
                <SortIcon
                  active={sortConfig.key === "ytd_performance_pct"}
                  direction={sortConfig.key === "ytd_performance_pct" ? sortConfig.direction : null}
                />
              </div>
            </th>
            <th className="m3-th text-right">{t("account.column_actions")}</th>
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <tr>
              <td colSpan={7} className="m3-td text-center py-12">
                <span className="text-m3-on-surface-variant animate-pulse">
                  {t("account.loading")}
                </span>
              </td>
            </tr>
          ) : isEmpty ? (
            // R11 — empty state distinct from no-search-results
            <tr>
              <td colSpan={7} className="m3-td text-center py-12 text-m3-on-surface-variant italic">
                {t("account.empty")}
              </td>
            </tr>
          ) : fetchError ? (
            // R12 — error state with retry (only shown when accounts exist but failed to reload)
            <tr>
              <td colSpan={7} className="m3-td text-center py-12">
                <div className="flex flex-col items-center gap-3">
                  <span className="text-m3-error text-sm">{t("account.error_load")}</span>
                  <Button variant="outline" size="sm" onClick={refetch}>
                    {t("action.retry")}
                  </Button>
                </div>
              </td>
            </tr>
          ) : hasNoSearchResults ? (
            // R10 — no search results (filter active, no match)
            <tr>
              <td colSpan={7} className="m3-td text-center py-12 text-m3-on-surface-variant italic">
                {t("account.no_search_results")}
              </td>
            </tr>
          ) : (
            sortedAndFilteredAccounts.map((account) => (
              <tr
                key={account.id}
                id={`account-row-${account.id}`}
                className="m3-tr cursor-pointer group hover:bg-m3-primary/5"
                tabIndex={0}
                aria-label={t("account.open_account", { name: account.name })}
                onClick={() => onAccountClick(account.id)}
                onKeyDown={(e) => handleRowKeyDown(e, account.id)}
              >
                <td className="m3-td">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-m3-on-surface">{account.name}</span>
                    {/* SYN-040 — the account holds an inconsistent holding after a merge */}
                    {account.has_inconsistent_holding && (
                      <AlertTriangle
                        id={`account-inconsistent-${account.id}`}
                        size={14}
                        className="shrink-0 text-m3-error"
                        role="img"
                        aria-label={t("account.inconsistent_holding")}
                      />
                    )}
                    <ChevronRight
                      size={14}
                      className="text-m3-primary opacity-0 group-hover:opacity-100 transition-opacity"
                    />
                  </div>
                </td>
                {/* ACC-026 — bank name from the account catalog; "—" when unset */}
                <td id={`account-bank-name-${account.id}`} className="m3-td">
                  <span
                    className={
                      account.bank_name === "" ? "text-m3-on-surface-variant" : "text-m3-on-surface"
                    }
                  >
                    {account.bank_name === "" ? "—" : account.bank_name}
                  </span>
                </td>
                <td className="m3-td">
                  <div className="flex items-center gap-2 text-m3-on-surface-variant">
                    <Calendar size={14} className="text-m3-primary" />
                    <span className="m3-chip-outline">
                      {t(FREQUENCY_I18N_KEYS[account.update_frequency])}
                    </span>
                  </div>
                </td>
                <td className="m3-td text-right tabular-nums">
                  <span className="font-medium text-m3-on-surface">
                    {microToFormatted(account.total_global_value, 2)}
                  </span>
                  <span className="ml-1 text-xs text-m3-on-surface-variant">
                    {account.currency}
                  </span>
                </td>
                {/* ACC-023 — account-wide unrealized P&L */}
                <td
                  id={`account-unrealized-pnl-${account.id}`}
                  className="m3-td text-right tabular-nums"
                >
                  <span
                    className={
                      account.total_unrealized_pnl == null || account.total_unrealized_pnl === 0
                        ? "text-m3-on-surface-variant"
                        : account.total_unrealized_pnl < 0
                          ? "text-m3-loss"
                          : "text-m3-gain"
                    }
                  >
                    {formatAccountRowTotalUnrealizedPnl(account.total_unrealized_pnl)}
                  </span>
                </td>
                {/* ACC-024 — year-to-date performance */}
                <td id={`account-ytd-pct-${account.id}`} className="m3-td text-right tabular-nums">
                  <span
                    className={
                      account.ytd_performance_pct == null || account.ytd_performance_pct === 0
                        ? "text-m3-on-surface-variant"
                        : account.ytd_performance_pct < 0
                          ? "text-m3-loss"
                          : "text-m3-gain"
                    }
                  >
                    {formatAccountRowYtdPerformancePct(account.ytd_performance_pct)}
                  </span>
                </td>
                <td className="m3-td text-right">
                  <div className="flex items-center justify-end gap-1">
                    <IconButton
                      icon={<Edit2 size={16} />}
                      variant="ghost"
                      id={`action-edit-account-${account.id}`}
                      aria-label={t("action.edit")}
                      onClick={(e) => handleEditClick(e, account)}
                    />
                    <IconButton
                      icon={<Trash2 size={16} />}
                      variant="danger"
                      id={`action-delete-account-${account.id}`}
                      aria-label={t("action.delete")}
                      disabled={fetchingSummaryFor === account.id}
                      onClick={(e) => handleDeleteClick(e, account.id, account.name)}
                    />
                  </div>
                </td>
              </tr>
            ))
          )}
        </tbody>
        {/* ACC-030/031 — the portfolio total closes the list, outside the sortable rows */}
        {showPortfolioTotal && portfolioTotal && (
          <tfoot>
            <tr
              id="account-portfolio-total"
              className="border-t-2 border-m3-outline/20 bg-m3-surface-container"
            >
              <td className="m3-td" colSpan={3}>
                <span className="font-semibold text-m3-on-surface">{t("account.total")}</span>
                {/* ACC-028 — an unconvertible account hides a figure from the total */}
                {portfolioTotal.incomplete && (
                  <span
                    id="account-portfolio-total-incomplete"
                    className="ml-2 text-xs text-m3-on-surface-variant"
                  >
                    {t("account.total_incomplete")}
                  </span>
                )}
              </td>
              <td id="account-portfolio-total-value" className="m3-td text-right tabular-nums">
                <span className="font-semibold text-m3-on-surface">
                  {microToFormatted(portfolioTotal.total_global_value, 2)}
                </span>
                <span className="ml-1 text-xs text-m3-on-surface-variant">
                  {portfolioTotal.currency}
                </span>
              </td>
              <td
                id="account-portfolio-total-unrealized-pnl"
                className="m3-td text-right tabular-nums"
              >
                <span
                  className={`font-semibold ${
                    portfolioTotal.total_unrealized_pnl == null ||
                    portfolioTotal.total_unrealized_pnl === 0
                      ? "text-m3-on-surface-variant"
                      : portfolioTotal.total_unrealized_pnl < 0
                        ? "text-m3-loss"
                        : "text-m3-gain"
                  }`}
                >
                  {formatAccountRowTotalUnrealizedPnl(portfolioTotal.total_unrealized_pnl)}
                </span>
              </td>
              <td className="m3-td" />
              <td className="m3-td" />
            </tr>
          </tfoot>
        )}
      </table>

      <EditAccountModal isOpen={!!editData} onClose={handleEditClose} account={editData} />

      {/* ACC-018 — standard confirmation dialog for accounts with no holdings */}
      <ConfirmationDialog
        isOpen={!!deleteData && (deleteSummary?.holding_count ?? 0) === 0}
        onCancel={handleDeleteCancel}
        onConfirm={handleDeleteConfirm}
        title={t("account.delete_confirm_title")}
        message={t("account.delete_confirm_message", {
          name: deleteData?.name ?? "",
        })}
        confirmLabel={t("action.delete")}
        cancelLabel={t("action.cancel")}
        variant="danger"
      />
      {/* ACC-019 — reinforced confirmation dialog for accounts with active holdings */}
      <ConfirmationDialog
        isOpen={!!deleteData && (deleteSummary?.holding_count ?? 0) > 0}
        onCancel={handleDeleteCancel}
        onConfirm={handleDeleteConfirm}
        title={t("account.delete_confirm_title")}
        message={t("account.delete_confirm_message_non_empty", {
          name: deleteData?.name ?? "",
          holdingCount: deleteSummary?.holding_count ?? 0,
          transactionCount: deleteSummary?.transaction_count ?? 0,
        })}
        confirmLabel={t("action.delete")}
        cancelLabel={t("action.cancel")}
        variant="danger"
      />
    </div>
  );
}

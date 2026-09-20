import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { SelectField } from "@/ui/components/field/SelectField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { EditTransactionModal } from "../edit_transaction_modal/EditTransactionModal";
import { routeEditTransaction } from "../shared/routeEditTransaction";
import { TransactionTable } from "../transaction_list/TransactionTable";
import { useTransactions } from "../useTransactions";
import { useAccountJournal } from "./useAccountJournal";

export function AccountJournalPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const showSnackbar = useSnackbar();
  const { cancelTransaction } = useTransactions();

  const {
    accountId,
    isLoading,
    error,
    sortDirection,
    filters,
    setFilter,
    clearFilters,
    toggleSortDirection,
    assetFilterOptions,
    typeFilterOptions,
    filteredSortedRows,
    transactionById,
    hasTransactions,
    refresh,
  } = useAccountJournal();

  const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null);
  const [deletingTransactionId, setDeletingTransactionId] = useState<string | null>(null);

  useEffect(() => {
    logger.info("[AccountJournalPage] mounted");
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!deletingTransactionId) return;
    const txAccountId = transactionById.get(deletingTransactionId)?.account_id ?? accountId;
    const { error: deleteError } = await cancelTransaction(deletingTransactionId, txAccountId);
    setDeletingTransactionId(null);
    if (deleteError) {
      showSnackbar(t("transaction.error_generic"), "error");
    } else {
      showSnackbar(t("transaction.success_deleted"), "success");
      await refresh();
    }
  }, [
    deletingTransactionId,
    transactionById,
    accountId,
    cancelTransaction,
    showSnackbar,
    t,
    refresh,
  ]);

  const allOption = { label: t("transaction.filter_all"), value: "" };

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden py-2 px-2">
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        {/* Filter bar */}
        <div className="px-6 py-4 bg-m3-surface-container-high flex flex-wrap gap-4 items-end">
          <div className="mr-auto flex items-center gap-3 self-center">
            <Link
              to="/accounts/$accountId"
              params={{ accountId }}
              id="journal-back"
              aria-label={t("action.back")}
              className="inline-flex items-center text-m3-on-surface-variant hover:text-m3-on-surface transition-colors"
            >
              <ArrowLeft size={18} />
            </Link>
            <h1 className="text-lg font-medium text-m3-on-surface">
              {t("transaction.journal_title")}
            </h1>
          </div>
          <div className="w-44">
            <SelectField
              id="journal-filter-asset"
              label={t("transaction.filter_asset_label")}
              value={filters.assetId}
              onChange={(e) => setFilter("assetId", e.target.value)}
              options={[allOption, ...assetFilterOptions]}
            />
          </div>
          <div className="w-44">
            <SelectField
              id="journal-filter-type"
              label={t("transaction.filter_type_label")}
              value={filters.type}
              onChange={(e) => setFilter("type", e.target.value)}
              options={[
                allOption,
                ...typeFilterOptions.map((o) => ({
                  value: o.value,
                  label: t(`transaction.type_${o.value.toLowerCase()}`),
                })),
              ]}
            />
          </div>
          <div className="w-32">
            <CalcField
              id="journal-filter-amount-min"
              label={t("transaction.filter_amount_min")}
              value={filters.amountMin}
              onValueChange={(v) => setFilter("amountMin", v)}
            />
          </div>
          <div className="w-32">
            <CalcField
              id="journal-filter-amount-max"
              label={t("transaction.filter_amount_max")}
              value={filters.amountMax}
              onValueChange={(v) => setFilter("amountMax", v)}
            />
          </div>
          <Button id="journal-clear-filters" variant="secondary" size="sm" onClick={clearFilters}>
            {t("action.reset")}
          </Button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto">
          {isLoading ? (
            <div className="animate-pulse p-4 space-y-3">
              {[1, 2, 3, 4].map((i) => (
                <div key={i} className="h-10 bg-m3-surface-variant rounded-lg" />
              ))}
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center h-full gap-3 py-12">
              <span className="text-m3-error text-sm">{t("transaction.error_load")}</span>
              <Button variant="secondary" size="sm" onClick={() => refresh()}>
                {t("action.retry")}
              </Button>
            </div>
          ) : !hasTransactions ? (
            <div className="flex items-center justify-center h-full py-12">
              <p className="text-m3-on-surface-variant italic">{t("transaction.journal_empty")}</p>
            </div>
          ) : filteredSortedRows.length === 0 ? (
            <div className="flex items-center justify-center h-full py-12">
              <p className="text-m3-on-surface-variant italic">
                {t("transaction.journal_no_match")}
              </p>
            </div>
          ) : (
            <TransactionTable
              rows={filteredSortedRows}
              sortDirection={sortDirection}
              onToggleSort={toggleSortDirection}
              showAssetColumn
              cashStatement
              onEditTransaction={(txId) => {
                const raw = transactionById.get(txId);
                if (raw) routeEditTransaction(navigate, raw, setEditingTransaction);
              }}
              onDeleteTransaction={setDeletingTransactionId}
            />
          )}
        </div>
      </div>

      {/* Edit modal — onClose only closes; onSuccess closes + refreshes */}
      {editingTransaction && (
        <EditTransactionModal
          isOpen
          onClose={() => setEditingTransaction(null)}
          onSuccess={() => {
            setEditingTransaction(null);
            void refresh();
          }}
          transaction={editingTransaction}
        />
      )}

      {/* Delete confirmation */}
      <ConfirmationDialog
        isOpen={!!deletingTransactionId}
        onCancel={() => setDeletingTransactionId(null)}
        onConfirm={handleConfirmDelete}
        title={t("transaction.delete_confirm_title")}
        message={t("transaction.delete_confirm_message")}
        confirmLabel={t("action.confirm")}
        cancelLabel={t("action.cancel")}
        variant="danger"
        confirmId="journal-delete-confirm"
      />
    </div>
  );
}

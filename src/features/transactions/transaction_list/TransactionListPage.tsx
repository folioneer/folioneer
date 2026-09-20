import { useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { SelectField } from "@/ui/components/field/SelectField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { AddTransactionModal } from "../add_transaction/AddTransactionModal";
import { EditTransactionModal } from "../edit_transaction_modal/EditTransactionModal";
import { routeEditTransaction } from "../shared/routeEditTransaction";
import { useTransactions } from "../useTransactions";
import { TransactionTable } from "./TransactionTable";
import { useTransactionList } from "./useTransactionList";

export function TransactionListPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { accountId: routeAccountId, assetId: routeAssetId } = useParams({
    from: "/accounts/$accountId/transactions/$assetId",
  });
  const { pendingTransactionAssetId } = useSearch({
    from: "/accounts/$accountId/transactions/$assetId",
  });

  useEffect(() => {
    logger.info("[TransactionListPage] mounted");
  }, []);
  const showSnackbar = useSnackbar();
  const { cancelTransaction } = useTransactions();

  const {
    selectedAccountId,
    selectedAssetId,
    accountOptions,
    assetOptions,
    isLoadingAssets,
    assetListError,
    isLoadingTransactions,
    transactionError,
    sortDirection,
    sortedTransactions,
    transactions,
    transactionById,
    handleAccountChange,
    handleAssetChange,
    toggleSortDirection,
    handleDeleteSuccess,
    handleEditSuccess,
    retryAssetList,
    retryTransactions,
  } = useTransactionList();

  const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null);
  const [deletingTransactionId, setDeletingTransactionId] = useState<string | null>(null);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [pendingAssetId, setPendingAssetId] = useState<string | undefined>(undefined);

  useEffect(() => {
    if (pendingTransactionAssetId) {
      setPendingAssetId(pendingTransactionAssetId);
      setIsAddModalOpen(true);
      navigate({
        to: "/accounts/$accountId/transactions/$assetId",
        params: { accountId: routeAccountId, assetId: routeAssetId },
        search: { pendingTransactionAssetId: undefined },
        replace: true,
      });
    }
  }, [pendingTransactionAssetId, routeAccountId, routeAssetId, navigate]);

  const handleCreateNewAsset = useCallback(
    (query: string) => {
      navigate({
        to: "/assets",
        search: {
          createNew: query,
          returnPath: `/accounts/${selectedAccountId}/transactions/${selectedAssetId ?? ""}`,
        },
      });
    },
    [navigate, selectedAccountId, selectedAssetId],
  );

  const handleConfirmDelete = useCallback(async () => {
    if (!deletingTransactionId) return;
    const accountId = transactionById.get(deletingTransactionId)?.account_id ?? selectedAccountId;
    const { error } = await cancelTransaction(deletingTransactionId, accountId);
    setDeletingTransactionId(null);
    if (error) {
      showSnackbar(t("transaction.error_generic"), "error");
    } else {
      showSnackbar(t("transaction.success_deleted"), "success");
      await handleDeleteSuccess();
    }
  }, [
    deletingTransactionId,
    transactionById,
    selectedAccountId,
    cancelTransaction,
    showSnackbar,
    t,
    handleDeleteSuccess,
  ]);

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden py-2 px-2">
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        {/* Filter bar */}
        <div className="px-6 py-4 bg-m3-surface-container-high flex flex-wrap gap-4 items-end">
          <div className="w-56">
            <SelectField
              id="txl-account"
              label={t("transaction.form_account_label")}
              value={selectedAccountId}
              onChange={(e) => handleAccountChange(e.target.value)}
              options={accountOptions}
            />
          </div>
          <div className="w-56">
            {isLoadingAssets ? (
              <div className="animate-pulse h-10 bg-m3-surface-variant rounded-xl" />
            ) : assetListError ? (
              <div className="flex flex-col gap-1">
                <span className="text-xs text-m3-error">{t("transaction.error_load_assets")}</span>
                <Button variant="secondary" size="sm" onClick={retryAssetList}>
                  {t("action.retry")}
                </Button>
              </div>
            ) : (
              <SelectField
                id="txl-asset"
                label={t("transaction.form_asset_label")}
                value={selectedAssetId ?? ""}
                onChange={(e) => handleAssetChange(e.target.value)}
                options={[{ label: `— ${t("action.select")} —`, value: "" }, ...assetOptions]}
              />
            )}
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto">
          {selectedAssetId === null ? (
            /* TXL-052 — incomplete filter prompt */
            <div className="flex items-center justify-center h-full py-12">
              <p className="text-m3-on-surface-variant italic">
                {t("transaction.select_asset_prompt")}
              </p>
            </div>
          ) : isLoadingTransactions ? (
            /* TXL-050 — loading skeletons */
            <div className="animate-pulse p-4 space-y-3">
              {[1, 2, 3, 4].map((i) => (
                <div key={i} className="h-10 bg-m3-surface-variant rounded-lg" />
              ))}
            </div>
          ) : transactionError ? (
            /* TXL-053 — transaction fetch error */
            <div className="flex flex-col items-center justify-center h-full gap-3 py-12">
              <span className="text-m3-error text-sm">{t("transaction.error_load")}</span>
              <Button variant="secondary" size="sm" onClick={retryTransactions}>
                {t("action.retry")}
              </Button>
            </div>
          ) : transactions.length === 0 ? (
            /* TXL-051 — empty state */
            <div className="flex flex-col items-center justify-center h-full gap-4 py-12">
              <p className="text-m3-on-surface-variant italic">
                {t("transaction.no_transactions")}
              </p>
              <Button
                variant="primary"
                size="sm"
                icon={<Plus size={14} />}
                onClick={() => setIsAddModalOpen(true)}
              >
                {t("account_details.add_transaction")}
              </Button>
            </div>
          ) : (
            /* TXL-022 — transaction table */
            <TransactionTable
              rows={sortedTransactions}
              sortDirection={sortDirection}
              onToggleSort={toggleSortDirection}
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
            handleEditSuccess();
          }}
          transaction={editingTransaction}
          onCreateNewAsset={handleCreateNewAsset}
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
        confirmId="txl-delete-confirm"
      />

      {/* Add transaction modal — empty state CTA */}
      <AddTransactionModal
        isOpen={isAddModalOpen}
        onClose={() => {
          setIsAddModalOpen(false);
          setPendingAssetId(undefined);
        }}
        prefillAccountId={selectedAccountId}
        prefillAssetId={pendingAssetId ?? selectedAssetId ?? undefined}
        onCreateNewAsset={handleCreateNewAsset}
      />
    </div>
  );
}

import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { DepositTransactionModal } from "@/features/account_details/deposit_transaction/DepositTransactionModal";
import { WithdrawalTransactionModal } from "@/features/account_details/withdrawal_transaction/WithdrawalTransactionModal";
import { transactionGateway } from "@/features/transactions/gateway";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";

/**
 * Shell-level URL-driven mount for editing a cash Deposit/Withdrawal (CSH-111).
 *
 * Subscribes to URL search params
 * (`modal=edit-cash-deposit|edit-cash-withdrawal&editTxId=…&editTxAccountId=…&editTxAssetId=…`)
 * and overlays the dedicated cash modal in edit mode. The transaction list (a
 * sibling feature) opens it by mutating URL params only — no cross-feature
 * import of the cash modals at the call site (CSH-111 / B13). The transaction is
 * (re)fetched here via the per-asset list command, then handed to the modal.
 */
export function CashTransactionEditMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const accounts = useAppStore((s) => s.accounts);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const editTxId = typeof search.editTxId === "string" ? search.editTxId : undefined;
  const accountId = typeof search.editTxAccountId === "string" ? search.editTxAccountId : undefined;
  const assetId = typeof search.editTxAssetId === "string" ? search.editTxAssetId : undefined;

  const isDeposit = modal === "edit-cash-deposit";
  const isWithdrawal = modal === "edit-cash-withdrawal";
  const active = (isDeposit || isWithdrawal) && !!editTxId && !!accountId && !!assetId;

  const [transaction, setTransaction] = useState<Transaction | null>(null);

  const handleClose = useCallback(() => {
    patchModalSearch(
      navigate,
      {
        modal: undefined,
        editTxId: undefined,
        editTxAccountId: undefined,
        editTxAssetId: undefined,
      },
      { replace: true },
    );
  }, [navigate]);

  useEffect(() => {
    if (!active || !accountId || !assetId || !editTxId) {
      setTransaction(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      const result = await transactionGateway.getTransactions(accountId, assetId);
      if (cancelled) return;
      if (result.status === "ok") {
        const found = result.data.find((tx) => tx.id === editTxId) ?? null;
        setTransaction(found);
        if (found === null) {
          // The transaction vanished (e.g. deleted in another tab) — drop the
          // stale modal params so the URL doesn't keep trying to open it.
          handleClose();
        }
      } else {
        // F27 — surface the failure instead of silently swallowing it, and
        // clear the modal params so the user isn't left with a dead URL state.
        logger.error("[CashTransactionEditMount] failed to load transaction", {
          error: result.error,
        });
        showSnackbar(t("error.Unknown"), "error");
        setTransaction(null);
        handleClose();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [active, accountId, assetId, editTxId, handleClose, showSnackbar, t]);

  if (!active || transaction === null || !accountId) return null;

  const account = accounts.find((a) => a.id === accountId);
  const accountName = account?.name ?? "";
  const accountCurrency = account?.currency ?? "";

  if (isDeposit) {
    return (
      <DepositTransactionModal
        isOpen
        onClose={handleClose}
        accountId={accountId}
        accountName={accountName}
        accountCurrency={accountCurrency}
        editTransaction={transaction}
        onSubmitSuccess={handleClose}
      />
    );
  }

  return (
    <WithdrawalTransactionModal
      isOpen
      onClose={handleClose}
      accountId={accountId}
      accountName={accountName}
      accountCurrency={accountCurrency}
      editTransaction={transaction}
      onSubmitSuccess={handleClose}
    />
  );
}

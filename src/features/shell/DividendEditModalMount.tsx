import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { EditDividendModal } from "@/features/account_details/dividend_transaction/EditDividendModal";
import { transactionGateway } from "@/features/transactions/gateway";
import { transactionLoadErrorToI18n } from "@/features/transactions/shared/presenter";
import { logger } from "@/lib/logger";
import { microToExactDecimal } from "@/lib/microUnits";
import { patchModalSearch } from "@/lib/modalSearch";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";

/**
 * Shell-level URL-driven mount for correcting a dividend (DIV-040).
 *
 * Subscribes to URL search params
 * (`modal=edit-dividend&editTxId=…&editTxAccountId=…&editTxAssetId=…`) and overlays the
 * dividend's own correction dialog. The transaction list (a sibling feature) opens it by
 * mutating URL params only — no cross-feature import at the call site (B13). The
 * transaction is (re)fetched here via the per-asset list command, then handed to the
 * dialog as what it corrects.
 */
export function DividendEditModalMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const assets = useAppStore((s) => s.assets);
  const accounts = useAppStore((s) => s.accounts);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const editTxId = typeof search.editTxId === "string" ? search.editTxId : undefined;
  const accountId = typeof search.editTxAccountId === "string" ? search.editTxAccountId : undefined;
  const assetId = typeof search.editTxAssetId === "string" ? search.editTxAssetId : undefined;

  const active = modal === "edit-dividend" && !!editTxId && !!accountId && !!assetId;

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
        // F27 — surface the failure instead of swallowing it; clear the modal
        // params so the user isn't left with a dead URL state.
        logger.error("[DividendEditModalMount] failed to load transaction", {
          error: result.error,
        });
        const message = transactionLoadErrorToI18n(result.error);
        showSnackbar(t(message.key, message.vars), "error");
        setTransaction(null);
        handleClose();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [active, accountId, assetId, editTxId, handleClose, showSnackbar, t]);

  if (!active || transaction === null || !accountId || !assetId) return null;

  const asset = assets.find((a) => a.id === assetId);
  const account = accounts.find((a) => a.id === accountId);
  // The dialog states both and takes their currencies: without them it would hide the
  // exchange rate or label it wrongly, so nothing is shown until the store holds them.
  if (!asset || !account) return null;

  return (
    <EditDividendModal
      onClose={handleClose}
      onSubmitSuccess={handleClose}
      accountName={account.name}
      accountCurrency={account.currency}
      assetName={asset.name}
      assetCurrency={asset.currency}
      // DIV-043 — between two currencies a stored rate of exactly 1 is how a dividend
      // typed in the account's currency is recorded: the correction keeps that mode.
      amountInAccountCurrency={
        asset.currency !== account.currency && transaction.exchange_rate === 1_000_000
      }
      dividend={{
        transactionId: transaction.id,
        accountId,
        assetId,
        date: transaction.date,
        // Exact, so saving without typing changes neither figure.
        amount: microToExactDecimal(transaction.quantity),
        exchangeRate: microToExactDecimal(transaction.exchange_rate),
        note: transaction.note ?? "",
        unitPriceMicro: transaction.unit_price,
      }}
    />
  );
}

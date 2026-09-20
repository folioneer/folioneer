import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { FreeSharesModal } from "@/features/account_details/free_shares_transaction/FreeSharesModal";
import { transactionGateway } from "@/features/transactions/gateway";
import { transactionLoadErrorToI18n } from "@/features/transactions/shared/presenter";
import { logger } from "@/lib/logger";
import { microToDecimal } from "@/lib/microUnits";
import { patchModalSearch } from "@/lib/modalSearch";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";

/**
 * Shell-level URL-driven mount for editing a free-share distribution (FSD-040).
 *
 * Subscribes to URL search params
 * (`modal=edit-free-shares&editTxId=…&editTxAccountId=…&editTxAssetId=…`) and
 * overlays the dedicated free-shares modal in edit mode (date/quantity/note
 * editable, asset locked). The transaction list (a sibling feature) opens it by
 * mutating URL params only — no cross-feature import at the call site (B13). The
 * transaction is (re)fetched here via the per-asset list command, then handed to
 * the modal as the prefill source.
 */
export function FreeSharesEditModalMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const assets = useAppStore((s) => s.assets);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const editTxId = typeof search.editTxId === "string" ? search.editTxId : undefined;
  const accountId = typeof search.editTxAccountId === "string" ? search.editTxAccountId : undefined;
  const assetId = typeof search.editTxAssetId === "string" ? search.editTxAssetId : undefined;

  const active = modal === "edit-free-shares" && !!editTxId && !!accountId && !!assetId;

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
        logger.error("[FreeSharesEditModalMount] failed to load transaction", {
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

  const assetName = assets.find((a) => a.id === assetId)?.name ?? "";

  return (
    <FreeSharesModal
      isOpen
      onClose={handleClose}
      accountId={accountId}
      heldAssets={[{ assetId, assetName, assetCurrency: "" }]}
      onSubmitSuccess={handleClose}
      editMode={{
        transactionId: transaction.id,
        lockedAssetId: assetId,
        lockedAssetName: assetName,
        initialDate: transaction.date,
        initialQuantity: microToDecimal(transaction.quantity),
        initialNote: transaction.note ?? "",
      }}
    />
  );
}

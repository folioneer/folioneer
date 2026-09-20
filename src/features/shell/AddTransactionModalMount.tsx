import { useNavigate, useRouterState, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { AddTransactionModal } from "@/features/transactions/add_transaction/AddTransactionModal";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";

/**
 * Shell-level URL-driven mount for the Add Transaction modal (ACD-035/036).
 *
 * Subscribes to `?modal=add-transaction` (+ optional `prefillAccountId`) and
 * overlays the transactions feature's `AddTransactionModal` in place. The
 * account-details FAB (a sibling feature) opens it by mutating URL params only —
 * no cross-feature import at the call site (B13); the shell is the one layer
 * allowed to import another feature's modal. Success closes the modal (the
 * transaction commit publishes `TransactionUpdated`, so the underlying view
 * re-fetches on its own).
 */
export function AddTransactionModalMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });

  useEffect(() => {
    logger.info("[AddTransactionModalMount] mounted");
  }, []);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const prefillAccountId =
    typeof search.prefillAccountId === "string" ? search.prefillAccountId : undefined;
  const active = modal === "add-transaction";

  const handleClose = useCallback(() => {
    patchModalSearch(
      navigate,
      { modal: undefined, prefillAccountId: undefined },
      { replace: true },
    );
  }, [navigate]);

  const handleCreateNewAsset = useCallback(
    (query: string) => {
      navigate({ to: "/assets", search: { createNew: query, returnPath: pathname } });
    },
    [navigate, pathname],
  );

  if (!active) return null;

  return (
    <AddTransactionModal
      isOpen
      onClose={handleClose}
      prefillAccountId={prefillAccountId}
      onCreateNewAsset={handleCreateNewAsset}
    />
  );
}

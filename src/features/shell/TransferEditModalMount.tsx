import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback } from "react";
import { TransferEditModal } from "@/features/account_details/transfer_transaction/TransferEditModal";
import { patchModalSearch } from "@/lib/modalSearch";

/**
 * Shell-level URL-driven mount for correcting a transfer (TRF-057).
 *
 * Subscribes to URL search params (`modal=edit-transfer&editTransferId=…`) and overlays the
 * transfer form for the whole transfer. The journals (a sibling feature) open it from any
 * part of the transfer by mutating URL params only — no cross-feature import at the call
 * site (B13).
 */
export function TransferEditModalMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const transferId = typeof search.editTransferId === "string" ? search.editTransferId : undefined;

  const handleClose = useCallback(() => {
    patchModalSearch(navigate, { modal: undefined, editTransferId: undefined }, { replace: true });
  }, [navigate]);

  if (modal !== "edit-transfer" || !transferId) return null;

  return (
    <TransferEditModal
      key={transferId}
      transferId={transferId}
      onClose={handleClose}
      onSubmitSuccess={handleClose}
    />
  );
}

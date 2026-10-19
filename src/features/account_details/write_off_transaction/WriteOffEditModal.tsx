import type { Transaction } from "@/bindings";
import { microToFieldDecimal } from "@/lib/microUnits";
import { writeOffAveragePriceMicro } from "../shared/presenter";
import type { WriteOffTarget } from "../shared/types";
import { useWriteOffEdit } from "./useWriteOffEdit";
import { WriteOffModal } from "./WriteOffModal";

interface WriteOffEditModalProps {
  accountId: string;
  /** The write-off corrected (WOF-058). */
  transaction: Transaction;
  assetName: string;
  assetReference: string;
  onClose: () => void;
  onSubmitSuccess: () => void;
}

/**
 * WOF-058 — the write-off form in edit mode, opened from a journal. Loads what the
 * account holds of the asset today (none once the write-off closed it); the average
 * price shown is the write-off's own, its cost over its quantity.
 */
export function WriteOffEditModal({
  accountId,
  transaction,
  assetName,
  assetReference,
  onClose,
  onSubmitSuccess,
}: WriteOffEditModalProps) {
  const heldMicro = useWriteOffEdit(accountId, transaction, assetReference, onClose);

  if (heldMicro === null) return null;

  const target: WriteOffTarget = {
    assetId: transaction.asset_id,
    assetName,
    assetReference,
    quantityMicro: heldMicro,
    averagePriceMicro: writeOffAveragePriceMicro(transaction),
  };

  return (
    <WriteOffModal
      isOpen
      onClose={onClose}
      accountId={accountId}
      target={target}
      onSubmitSuccess={onSubmitSuccess}
      editMode={{
        transactionId: transaction.id,
        initialDate: transaction.date,
        initialQuantity: microToFieldDecimal(transaction.quantity),
        initialQuantityMicro: transaction.quantity,
        initialReason: transaction.write_off_reason ?? "Other",
        initialNote: transaction.note ?? "",
      }}
    />
  );
}

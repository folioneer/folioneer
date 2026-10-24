import { microToFieldDecimal } from "@/lib/microUnits";
import { otherAccounts, useAppStore } from "@/lib/store";
import { transferAveragePriceMicro } from "../shared/presenter";
import type { TransferTarget } from "../shared/types";
import { TransferModal } from "./TransferModal";
import { useTransferEdit } from "./useTransferEdit";

interface TransferEditModalProps {
  /** The transfer corrected as one (TRF-057). */
  transferId: string;
  onClose: () => void;
  onSubmitSuccess: () => void;
}

/**
 * TRF-057 — the transfer form for the whole transfer, opened from any of its parts in a
 * journal. Reads the transfer as one (TRF-060) and what the sending account holds today;
 * the average price shown is the transfer out's own. A failed read shows its message and
 * closes the form.
 */
export function TransferEditModal({
  transferId,
  onClose,
  onSubmitSuccess,
}: TransferEditModalProps) {
  const accounts = useAppStore((s) => s.accounts);
  const assets = useAppStore((s) => s.assets);
  const loaded = useTransferEdit(transferId, onClose);

  if (loaded === null) return null;

  const { transfer, heldMicro } = loaded;
  const asset = assets.find((a) => a.id === transfer.asset_id);
  const from = accounts.find((a) => a.id === transfer.from_account_id);
  const destinations = otherAccounts(accounts, transfer.from_account_id);
  const target: TransferTarget = {
    assetId: transfer.asset_id,
    assetName: asset?.name ?? "",
    assetReference: asset?.reference ?? "",
    quantityMicro: heldMicro,
    averagePriceMicro: transferAveragePriceMicro(transfer),
    accountName: from?.name ?? "",
    accountCurrency: from?.currency ?? "",
  };

  return (
    <TransferModal
      isOpen
      onClose={onClose}
      accountId={transfer.from_account_id}
      target={target}
      destinations={destinations}
      onSubmitSuccess={onSubmitSuccess}
      editMode={{
        transferId: transfer.transfer_id,
        toAccountId: transfer.to_account_id,
        initialDate: transfer.date,
        initialSent: microToFieldDecimal(transfer.quantity_sent),
        initialReceived: microToFieldDecimal(transfer.quantity_received),
        initialRate: microToFieldDecimal(transfer.exchange_rate),
        initialSentMicro: transfer.quantity_sent,
        initialNote: transfer.note ?? "",
      }}
    />
  );
}

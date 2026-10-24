import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TransferView } from "@/bindings";
import { logger } from "@/lib/logger";
import { microToFieldDecimal } from "@/lib/microUnits";
import { otherAccounts, useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { transferErrorToI18n } from "../shared/presenter";
import type { TransferTarget } from "../shared/types";
import { TransferModal } from "./TransferModal";

interface TransferEditModalProps {
  /** The transfer corrected as one (TRF-057). */
  transferId: string;
  onClose: () => void;
  onSubmitSuccess: () => void;
}

interface Loaded {
  transfer: TransferView;
  /** What the sending account holds of the asset today, the transfer's sending side apart. */
  heldMicro: number;
}

/**
 * TRF-057 — the transfer form for the whole transfer, opened from any of its parts in a
 * journal. Reads the transfer as one (TRF-060) and what the sending account holds today;
 * the average price shown is the transfer's own, its cost over its quantity sent. A failed
 * read shows its message and closes the form.
 */
export function TransferEditModal({
  transferId,
  onClose,
  onSubmitSuccess,
}: TransferEditModalProps) {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();
  const accounts = useAppStore((s) => s.accounts);
  const assets = useAppStore((s) => s.assets);
  const [loaded, setLoaded] = useState<Loaded | null>(null);

  useEffect(() => {
    let cancelled = false;
    const fail = (message: { key: string; vars?: Record<string, string | number> }) => {
      showSnackbar(t(message.key, message.vars), "error");
      onClose();
    };
    void (async () => {
      const read = await accountDetailsGateway.getTransfer(transferId);
      if (cancelled) return;
      if (read.status === "error") {
        logger.error("[TransferEditModal] the transfer was not read", { error: read.error });
        fail(transferErrorToI18n(read.error, "", "", i18n.language).message);
        return;
      }
      const transfer = read.data;
      const details = await accountDetailsGateway.getAccountDetails(transfer.from_account_id);
      if (cancelled) return;
      if (details.status === "error") {
        logger.error("[TransferEditModal] the holding was not read", { error: details.error });
        fail({ key: `error.${details.error.code}` });
        return;
      }
      const holding = details.data.holdings.find((h) => h.asset_id === transfer.asset_id);
      setLoaded({ transfer, heldMicro: holding?.quantity ?? 0 });
    })();
    return () => {
      cancelled = true;
    };
  }, [transferId, onClose, showSnackbar, t, i18n.language]);

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
    averagePriceMicro: Number(
      (BigInt(transfer.cost) * 1_000_000n) / BigInt(transfer.quantity_received),
    ),
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

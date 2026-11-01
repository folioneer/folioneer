import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TransferView } from "@/bindings";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { transferErrorToI18n } from "../shared/presenter";

export interface TransferEditLoaded {
  transfer: TransferView;
  /** What the sending account holds of the asset today, the transfer's sending side apart. */
  heldQuantity: string;
}

/**
 * TRF-057 — the transfer read as one (TRF-060), then what its sending account holds of the
 * asset today, for the edit form; null while they load. A failed read shows its message and
 * closes the form.
 */
export function useTransferEdit(
  transferId: string,
  onClose: () => void,
): TransferEditLoaded | null {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();
  const [loaded, setLoaded] = useState<TransferEditLoaded | null>(null);

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
        logger.error("[useTransferEdit] the transfer was not read", { error: read.error });
        fail(transferErrorToI18n(read.error, "", "", i18n.language).message);
        return;
      }
      const transfer = read.data;
      const details = await accountDetailsGateway.getAccountDetails(transfer.from_account_id);
      if (cancelled) return;
      if (details.status === "error") {
        logger.error("[useTransferEdit] the holding was not read", { error: details.error });
        fail({ key: `error.${details.error.code}` });
        return;
      }
      const holding = details.data.holdings.find((h) => h.asset_id === transfer.asset_id);
      setLoaded({ transfer, heldQuantity: holding?.quantity ?? "0" });
    })();
    return () => {
      cancelled = true;
    };
  }, [transferId, onClose, showSnackbar, t, i18n.language]);

  return loaded;
}

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { microToFieldDecimal } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { writeOffAveragePriceMicro, writeOffErrorToI18n } from "../shared/presenter";
import type { WriteOffTarget } from "../shared/types";
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
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [heldMicro, setHeldMicro] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const result = await accountDetailsGateway.getAccountDetails(accountId);
      if (cancelled) return;
      if (result.status === "error") {
        logger.error("[WriteOffEditModal] failed to load the holding", { error: result.error });
        const { message } = writeOffErrorToI18n(result.error, assetReference);
        showSnackbar(t(message.key, message.vars), "error");
        onClose();
        return;
      }
      const holding = result.data.holdings.find((h) => h.asset_id === transaction.asset_id);
      setHeldMicro(holding?.quantity ?? 0);
    })();
    return () => {
      cancelled = true;
    };
  }, [accountId, transaction.asset_id, assetReference, onClose, showSnackbar, t]);

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

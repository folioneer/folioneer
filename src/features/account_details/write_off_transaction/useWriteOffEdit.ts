import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { writeOffErrorToI18n } from "../shared/presenter";

/**
 * WOF-058 — what the account holds of the written-off asset today, for the edit form
 * (0 once the write-off closed it); null while it loads. A failed read shows its message
 * and closes the form.
 */
export function useWriteOffEdit(
  accountId: string,
  transaction: Transaction,
  assetReference: string,
  onClose: () => void,
): string | null {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();
  const [heldQuantity, setHeldQuantity] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const result = await accountDetailsGateway.getAccountDetails(accountId);
      if (cancelled) return;
      if (result.status === "error") {
        logger.error("[useWriteOffEdit] failed to load the holding", { error: result.error });
        const { message } = writeOffErrorToI18n(
          result.error,
          assetReference,
          transaction.date,
          i18n.language,
        );
        showSnackbar(t(message.key, message.vars), "error");
        onClose();
        return;
      }
      const holding = result.data.holdings.find((h) => h.asset_id === transaction.asset_id);
      setHeldQuantity(holding?.quantity ?? "0");
    })();
    return () => {
      cancelled = true;
    };
  }, [
    accountId,
    transaction.asset_id,
    transaction.date,
    assetReference,
    onClose,
    showSnackbar,
    t,
    i18n.language,
  ]);

  return heldQuantity;
}

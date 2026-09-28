import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Asset } from "@/bindings";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { transactionGateway } from "../gateway";

/**
 * TRX-064 — the assets a purchase or a sale is recorded on, as the core lists them (every
 * asset but the Cash Assets, CSH-018). Loaded on mount and again when assets change, so an
 * asset created from the form appears in it. A failed load shows the generic error.
 */
export function useNonCashAssets(): Asset[] {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [assets, setAssets] = useState<Asset[]>([]);

  const load = useCallback(async () => {
    const fail = (error: unknown) => {
      logger.error("Failed to load the assets a transaction is recorded on", { error });
      showSnackbar(t("transaction.error_generic"), "error");
    };
    try {
      const res = await transactionGateway.getNonCashAssets();
      if (res.status === "ok") setAssets(res.data);
      else fail(res.error);
    } catch (e) {
      fail(e);
    }
  }, [t, showSnackbar]);

  useEffect(() => {
    load();
    const unlistenPromise = transactionGateway.subscribeToEvents((type) => {
      if (type === "AssetUpdated" || type === "SyncCompleted") load();
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [load]);

  return assets;
}

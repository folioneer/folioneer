import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountGateway } from "../gateway";
import { fetchPriceErrorToI18n } from "../shared/presenter";

/**
 * MKT-115 / MKT-133 — Global "Refresh prices" hook for the AccountManager header.
 *
 * Calls `accountGateway.fetchAllAssetPrices()` and surfaces the result via the
 * global snackbar. Success dispatches `mkt.fetch_dispatched`; errors route
 * through `fetchPriceErrorToI18n` (F27 layer 3) for typed key + severity.
 * Keyless (ADR-017): no API key, so no key gate — the fetch dispatches directly.
 *
 * `isPending` toggles for the duration of the gateway call so the button can disable itself.
 */
export function useRefreshGlobalPrices(): {
  isPending: boolean;
  refresh: () => Promise<void>;
} {
  const [isPending, setIsPending] = useState(false);
  const showSnackbar = useSnackbar();
  const { t } = useTranslation();

  const refresh = useCallback(async () => {
    setIsPending(true);
    try {
      // PMV-010 — the user asked for this one, so it is the path that reports movement.
      const result = await accountGateway.fetchAllAssetPrices("Manual");
      if (result.status === "ok") {
        showSnackbar(t("mkt.fetch_dispatched"), "info");
        return;
      }
      const msg = fetchPriceErrorToI18n(result.error);
      showSnackbar(t(msg.key, msg.vars), msg.severity);
    } finally {
      setIsPending(false);
    }
  }, [showSnackbar, t]);

  return { isPending, refresh };
}

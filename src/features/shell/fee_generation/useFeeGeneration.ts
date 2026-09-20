import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { shellGateway } from "../gateway";

/**
 * FEE-040 — lazy catch-up generation. Fires `applyDueFeeDeductions` once on app
 * mount so any recurring schedules backfill their due periods before the user
 * navigates. Each generated deduction emits `TransactionUpdated`, which the
 * account-details view already listens for, so the UI refreshes itself. A
 * failure surfaces via the snackbar (F27) — generation is best-effort, so it
 * does not block the app.
 */
export function useFeeGeneration(): void {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  useEffect(() => {
    shellGateway.applyDueFeeDeductions().then((result) => {
      if (result.status === "error") {
        logger.error("[useFeeGeneration] applyDueFeeDeductions failed", { error: result.error });
        showSnackbar(t("fee_generation.apply_error"), "error");
      }
    });
  }, [t, showSnackbar]);
}

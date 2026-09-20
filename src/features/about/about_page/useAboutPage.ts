import { useCallback, useEffect, useState } from "react";
import { logger } from "@/lib/logger";
import { updateGateway } from "@/lib/updateGateway";

export type CheckStatus = "idle" | "checking" | "up_to_date" | "error";

export interface AboutPageData {
  /** Status of the last manual update check (R26, R27). */
  checkStatus: CheckStatus;
  /** Triggers a manual update check (R25). */
  handleCheckForUpdate: () => Promise<void>;
}

export function useAboutPage(): AboutPageData {
  const [checkStatus, setCheckStatus] = useState<CheckStatus>("idle");

  useEffect(() => {
    logger.info("[AboutPage] mounted");
  }, []);

  // R25 — manual check; R26 — button disabled while checking
  const handleCheckForUpdate = useCallback(async () => {
    if (checkStatus === "checking") return;
    setCheckStatus("checking");
    try {
      const info = await updateGateway.checkForUpdate();
      // R27 — if update found, banner shows automatically via update:available event
      // If no update, show "up to date" message
      setCheckStatus(info ? "idle" : "up_to_date");
    } catch (e) {
      logger.error("[AboutPage] Manual check failed", e);
      setCheckStatus("error");
    }
  }, [checkStatus]);

  return { checkStatus, handleCheckForUpdate };
}

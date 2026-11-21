import { useCallback, useEffect, useState } from "react";
import { logger } from "@/lib/logger";
import { aboutGateway } from "../gateway";

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
      const result = await aboutGateway.checkForUpdate();
      if (result.status === "error") {
        // #873's mock — any failed check says so, a refusal too; the banner names a refusal
        logger.error("[AboutPage] Manual check refused", result.error);
        setCheckStatus("error");
        return;
      }
      // R27 — if update found, banner shows automatically via update:available event
      // If no update, show "up to date" message
      setCheckStatus(result.data ? "idle" : "up_to_date");
    } catch (e) {
      logger.error("[AboutPage] Manual check failed", e);
      setCheckStatus("error");
    }
  }, [checkStatus]);

  return { checkStatus, handleCheckForUpdate };
}

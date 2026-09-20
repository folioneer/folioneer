import { useCallback, useEffect, useState } from "react";
import type { PriceMovementReport } from "@/bindings";
import { accountGateway } from "../gateway";

export interface UsePriceMovementReportResult {
  /** The report from the most recent Global refresh, or `null` when there is none. */
  report: PriceMovementReport | null;
  /** PMV-061 — drops the report; nothing brings it back. */
  dismiss: () => void;
}

/**
 * PMV-013/016/061 — holds the Price Movement report for the surface that is
 * currently mounted.
 *
 * The subscription is deliberately mount-scoped rather than app-wide: PMV-016
 * says the report belongs to the accounts list, so a user who navigated away
 * before the refresh finished gets none and none is kept for their return. An
 * always-on sink would retain it across navigation and quietly break that rule.
 *
 * A payload whose `movement` is null carries no report — the Launch auto-fetch
 * (PMV-010) and a report that could not be produced (PMV-014) both look like
 * that, and neither should disturb what is on screen.
 */
export function usePriceMovementReport(): UsePriceMovementReportResult {
  const [report, setReport] = useState<PriceMovementReport | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void accountGateway
      .subscribeToPriceFetchCompleted((payload) => {
        if (payload.movement !== null) {
          setReport(payload.movement);
        }
      })
      .then((dispose) => {
        if (cancelled) {
          dispose();
          return;
        }
        unlisten = dispose;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const dismiss = useCallback(() => setReport(null), []);

  return { report, dismiss };
}

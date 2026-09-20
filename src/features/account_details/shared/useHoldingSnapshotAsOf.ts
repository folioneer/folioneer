import { useEffect, useMemo, useState } from "react";
import type { AccountError, HoldingSnapshot } from "@/bindings";
import { accountDetailsGateway } from "../gateway";

/** Local calendar date as ISO `YYYY-MM-DD` — the trade-dialog default when no date is entered. */
function todayIso(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * TDI-010 — fetches the (account, asset) holding snapshot (quantity + VWAP
 * average cost) as of `date`, defaulting to today when `date` is empty (TDI-020).
 * `snapshot` is null until the first load resolves or when the query fails; the
 * typed `error` is surfaced (not silently dropped, F27) so callers can react —
 * the trade dialogs simply show nothing on error.
 */
export function useHoldingSnapshotAsOf(
  accountId: string,
  assetId: string,
  date: string,
): { snapshot: HoldingSnapshot | null; error: AccountError | null } {
  const [snapshot, setSnapshot] = useState<HoldingSnapshot | null>(null);
  const [error, setError] = useState<AccountError | null>(null);
  const effectiveDate = useMemo(() => date || todayIso(), [date]);

  useEffect(() => {
    let cancelled = false;
    accountDetailsGateway.getHoldingSnapshotAsOf(accountId, assetId, effectiveDate).then((res) => {
      if (cancelled) return;
      if (res.status === "ok") {
        setSnapshot(res.data);
        setError(null);
      } else {
        setSnapshot(null);
        setError(res.error);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [accountId, assetId, effectiveDate]);

  return { snapshot, error };
}

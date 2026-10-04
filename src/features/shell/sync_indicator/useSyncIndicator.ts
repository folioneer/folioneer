import { useCallback, useEffect, useState } from "react";
import type { SyncStatusView } from "@/bindings";
import { logger } from "@/lib/logger";
import { getSyncStatus, onSyncCompleted } from "../gateway";

export interface UseSyncIndicatorResult {
  isLoading: boolean;
  /** False while sync is disabled on this device (SYN-010). */
  visible: boolean;
  lastSyncCompletedAt: string | null;
  /** True when the core says sync needs attention (SYN-063). */
  needsAttention: boolean;
}

/**
 * SYN-063/064 — compact shell indicator state: reads the sync status on mount
 * and again after every `SyncCompleted` marker event.
 */
export function useSyncIndicator(): UseSyncIndicatorResult {
  const [isLoading, setIsLoading] = useState(true);
  const [status, setStatus] = useState<SyncStatusView | null>(null);

  const refresh = useCallback(async () => {
    const result = await getSyncStatus();
    if (result.status === "ok") {
      setStatus(result.data);
    } else {
      logger.error("[useSyncIndicator] get_sync_status failed", { error: result.error });
    }
    setIsLoading(false);
  }, []);

  useEffect(() => {
    void refresh();
    const unlistenPromise = onSyncCompleted(() => void refresh());
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

  return {
    isLoading,
    visible: status?.enabled === true,
    lastSyncCompletedAt: status?.last_sync_completed_at ?? null,
    needsAttention: status?.health === "needs_attention",
  };
}

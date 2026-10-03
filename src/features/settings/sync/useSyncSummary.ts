import { useEffect, useState } from "react";
import type { I18nMessage } from "@/ui/format/i18n";
import { getSyncStatus } from "../gateway";
import { syncErrorToI18n } from "../shared/presenter";

export interface UseSyncSummaryResult {
  /** Which of `sync.status_*` says the state of sync on this computer; null until read. */
  stateKey: string | null;
  /** Set when the status could not be read (F27). */
  loadError: I18nMessage | null;
}

/** SYN-063 — the one line the settings keep about sync: not enabled, enabled or paused. */
export function useSyncSummary(): UseSyncSummaryResult {
  const [stateKey, setStateKey] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    void getSyncStatus().then((result) => {
      if (result.status === "ok") {
        const { enabled, paused } = result.data;
        setStateKey(
          !enabled ? "sync.status_disabled" : paused ? "sync.status_paused" : "sync.status_enabled",
        );
      } else {
        setLoadError(syncErrorToI18n(result.error));
      }
    });
  }, []);

  return { stateKey, loadError };
}

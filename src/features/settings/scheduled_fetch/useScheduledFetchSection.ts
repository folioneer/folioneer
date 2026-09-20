import { useCallback, useEffect, useState } from "react";
import type { ScheduledFetchRun } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { configureScheduledFetch, getScheduledFetchStatus } from "../gateway";
import { scheduledFetchErrorToI18n } from "../shared/presenter";

/** SPF-018 — local default trigger time before any configuration has loaded. */
export const DEFAULT_TRIGGER_TIME = "22:15";

export interface UseScheduledFetchSectionResult {
  /** SPF-061 — true while the initial status load is in flight. */
  isLoading: boolean;
  /** SPF-061 — set when the initial status load fails; the rest of Settings is unaffected. */
  loadError: I18nMessage | null;
  enabled: boolean;
  triggerTime: string;
  lastRun: ScheduledFetchRun | null;
  /** SPF-060 — true while a configure() call is being acknowledged. */
  isConfiguring: boolean;
  /** SPF-013 — set when a configure() call is rejected; enabled/triggerTime revert. */
  configureError: I18nMessage | null;
  configure: (enabled: boolean, triggerTime: string) => Promise<void>;
}

/**
 * SPF-010/012/013/018/052/060/061 — Settings section state: loads the
 * scheduled-fetch status on mount and exposes `configure` with in-flight
 * tracking and revert-on-error. After a successful configure only the last
 * run is re-read — the just-applied configuration stays authoritative over
 * the re-read snapshot (SPF-024: no live events).
 */
export function useScheduledFetchSection(): UseScheduledFetchSectionResult {
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<I18nMessage | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [triggerTime, setTriggerTime] = useState(DEFAULT_TRIGGER_TIME);
  const [lastRun, setLastRun] = useState<ScheduledFetchRun | null>(null);
  const [isConfiguring, setIsConfiguring] = useState(false);
  const [configureError, setConfigureError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const result = await getScheduledFetchStatus();
      if (cancelled) return;
      if (result.status === "ok") {
        setEnabled(result.data.configuration.enabled);
        setTriggerTime(result.data.configuration.trigger_time);
        setLastRun(result.data.last_run);
        setLoadError(null);
      } else {
        setLoadError(scheduledFetchErrorToI18n(result.error));
      }
      setIsLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const configure = useCallback(
    async (nextEnabled: boolean, nextTriggerTime: string) => {
      const priorEnabled = enabled;
      const priorTriggerTime = triggerTime;
      setIsConfiguring(true);
      setConfigureError(null);
      setEnabled(nextEnabled);
      setTriggerTime(nextTriggerTime);

      const result = await configureScheduledFetch(nextEnabled, nextTriggerTime);
      if (result.status === "ok") {
        const status = await getScheduledFetchStatus();
        if (status.status === "ok") {
          setLastRun(status.data.last_run);
        } else {
          // F27 — the best-effort refresh failure is surfaced, never dropped;
          // the applied configuration itself succeeded and stays in place.
          setConfigureError(scheduledFetchErrorToI18n(status.error));
        }
      } else {
        // SPF-013 — the toggle and time revert to their prior values.
        setEnabled(priorEnabled);
        setTriggerTime(priorTriggerTime);
        setConfigureError(scheduledFetchErrorToI18n(result.error));
      }
      setIsConfiguring(false);
    },
    [enabled, triggerTime],
  );

  return {
    isLoading,
    loadError,
    enabled,
    triggerTime,
    lastRun,
    isConfiguring,
    configureError,
    configure,
  };
}

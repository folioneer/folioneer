import { useTranslation } from "react-i18next";
import { formatIsoDateTime } from "@/ui/format/date";
import { formatScheduledFetchStatusLine } from "../shared/presenter";
import { useScheduledFetchSection } from "./useScheduledFetchSection";

/**
 * SPF-010/013/018/052/060/061 — Settings section: enable toggle, trigger-time
 * field (editable only while enabled), last-run status line, loading/in-flight/
 * error states. Stable ids per F25: `scheduled-fetch-toggle`,
 * `scheduled-fetch-time`, `scheduled-fetch-status`.
 */
export function ScheduledFetchSection() {
  const { t, i18n } = useTranslation();
  const {
    isLoading,
    loadError,
    enabled,
    triggerTime,
    lastRun,
    isConfiguring,
    configureError,
    configure,
  } = useScheduledFetchSection();

  const statusLine = formatScheduledFetchStatusLine(lastRun);
  const statusVars =
    typeof statusLine.vars?.executedAt === "string"
      ? {
          ...statusLine.vars,
          executedAt: formatIsoDateTime(statusLine.vars.executedAt, i18n.language),
        }
      : statusLine.vars;

  return (
    <section className="flex flex-col gap-2">
      <label className="flex items-start gap-3 cursor-pointer group">
        <input
          id="scheduled-fetch-toggle"
          data-testid="scheduled-fetch-toggle"
          type="checkbox"
          checked={enabled}
          disabled={isConfiguring || isLoading}
          onChange={() => void configure(!enabled, triggerTime)}
          className="accent-m3-primary w-4 h-4 mt-1"
        />
        <span className="flex flex-col gap-1">
          <span className="text-sm font-medium text-m3-on-surface group-hover:text-m3-primary transition-colors">
            {t("settings.scheduled_fetch_label")}
          </span>
          <span className="text-xs text-m3-on-surface-variant">
            {t("settings.scheduled_fetch_description")}
          </span>
        </span>
      </label>

      {enabled && (
        <label className="flex items-center gap-3 pl-7">
          <span className="text-sm text-m3-on-surface-variant">
            {t("settings.scheduled_fetch_time_label")}
          </span>
          <input
            id="scheduled-fetch-time"
            data-testid="scheduled-fetch-time"
            type="time"
            value={triggerTime}
            disabled={isConfiguring}
            onChange={(event) => void configure(enabled, event.target.value)}
            className="rounded-lg border border-m3-outline-variant bg-m3-surface px-2 py-1 text-sm text-m3-on-surface"
          />
        </label>
      )}

      {isLoading && (
        <span
          data-testid="scheduled-fetch-loading"
          className="pl-7 text-xs text-m3-on-surface-variant"
        >
          {t("settings.scheduled_fetch.loading")}
        </span>
      )}

      {!isLoading && loadError && (
        <span data-testid="scheduled-fetch-load-error" className="pl-7 text-xs text-m3-error">
          {t(loadError.key, loadError.vars)}
        </span>
      )}

      {!isLoading && !loadError && enabled && (
        <span
          id="scheduled-fetch-status"
          data-testid="scheduled-fetch-status"
          className="pl-7 text-xs text-m3-on-surface-variant"
        >
          {t(statusLine.key, statusVars)}
        </span>
      )}

      {configureError && (
        <span data-testid="scheduled-fetch-configure-error" className="pl-7 text-xs text-m3-error">
          {t(configureError.key, configureError.vars)}
        </span>
      )}
    </section>
  );
}

import { useTranslation } from "react-i18next";
import { CheckboxField } from "@/ui/components/field/CheckboxField";
import { TextField } from "@/ui/components/field/TextField";
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
      <CheckboxField
        id="scheduled-fetch-toggle"
        label={t("settings.scheduled_fetch_label")}
        description={t("settings.scheduled_fetch_description")}
        checked={enabled}
        disabled={isConfiguring || isLoading}
        onChange={(checked) => void configure(checked, triggerTime)}
      />

      {enabled && (
        <div className="w-44">
          <TextField
            id="scheduled-fetch-time"
            data-testid="scheduled-fetch-time"
            label={t("settings.scheduled_fetch_time_label")}
            type="time"
            value={triggerTime}
            disabled={isConfiguring}
            onChange={(event) => void configure(enabled, event.target.value)}
          />
        </div>
      )}

      {isLoading && (
        <span data-testid="scheduled-fetch-loading" className="text-xs text-m3-on-surface-variant">
          {t("settings.scheduled_fetch.loading")}
        </span>
      )}

      {!isLoading && loadError && (
        <span data-testid="scheduled-fetch-load-error" className="text-xs text-m3-error">
          {t(loadError.key, loadError.vars)}
        </span>
      )}

      {!isLoading && !loadError && enabled && (
        <span
          id="scheduled-fetch-status"
          data-testid="scheduled-fetch-status"
          className="text-xs text-m3-on-surface-variant"
        >
          {t(statusLine.key, statusVars)}
        </span>
      )}

      {isConfiguring && (
        <span
          id="scheduled-fetch-pending"
          role="status"
          className="text-xs text-m3-on-surface-variant"
        >
          {t("settings.scheduled_fetch.saving")}
        </span>
      )}

      {configureError && (
        <span data-testid="scheduled-fetch-configure-error" className="text-xs text-m3-error">
          {t(configureError.key, configureError.vars)}
        </span>
      )}
    </section>
  );
}

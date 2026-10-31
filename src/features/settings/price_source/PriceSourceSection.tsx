import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CheckboxField } from "@/ui/components/field/CheckboxField";
import { SelectField } from "@/ui/components/field/SelectField";
import { SOURCE_OFF, usePriceSource } from "./usePriceSource";

/**
 * MKT-215 — the price source of this computer: Off or a provider the build offers, which
 * becomes the choice once its notice is acknowledged. Stable ids per F25:
 * `settings-price-source`, `settings-price-source-notice`, `settings-price-source-understood`.
 */
export function PriceSourceSection() {
  const { t } = useTranslation();
  const {
    choice,
    selected,
    awaitingAcknowledgement,
    isLoading,
    isSaving,
    error,
    select,
    acknowledge,
  } = usePriceSource();

  return (
    <section className="flex flex-col gap-3">
      {choice && (
        <SelectField
          id="settings-price-source"
          label={t("settings.price_source_label")}
          value={selected}
          disabled={isSaving}
          onChange={(event) => select(event.target.value)}
          options={[
            { value: SOURCE_OFF, label: t("settings.price_source_off") },
            ...choice.offered.map((id) => ({
              value: id,
              label: t(`settings.price_source_option.${id}`),
            })),
          ]}
        />
      )}

      {awaitingAcknowledgement === null && choice?.chosen && (
        <span id="settings-price-source-note" className="text-xs text-m3-on-surface-variant">
          {t(`settings.price_source_note.${choice.chosen}`)}
        </span>
      )}

      {awaitingAcknowledgement !== null && (
        <>
          <div
            id="settings-price-source-notice"
            className="flex items-start gap-3 rounded-xl bg-m3-warning-container p-3 text-sm text-m3-on-warning-container"
          >
            <AlertTriangle size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
            <span>{t(`settings.price_source_notice.${awaitingAcknowledgement}`)}</span>
          </div>
          <CheckboxField
            id="settings-price-source-understood"
            label={t("settings.price_source_understood_label")}
            description={t("settings.price_source_understood_description")}
            checked={isSaving}
            disabled={isSaving}
            onChange={acknowledge}
          />
        </>
      )}

      {(isLoading || isSaving) && (
        <span
          id="settings-price-source-pending"
          role="status"
          className="text-xs text-m3-on-surface-variant"
        >
          {t(isSaving ? "settings.price_source_saving" : "settings.price_source_loading")}
        </span>
      )}

      {error && (
        <span id="settings-price-source-error" role="alert" className="text-xs text-m3-error">
          {t(error.key, error.vars)}
        </span>
      )}
    </section>
  );
}

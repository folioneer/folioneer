import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { ScheduledFetchSection } from "./scheduled_fetch/ScheduledFetchSection";
import { SyncSection } from "./sync/SyncSection";
import { type LanguageChoice, useSettings } from "./useSettings";

const LANGUAGE_OPTIONS: { value: LanguageChoice; labelKey: string }[] = [
  { value: "auto", labelKey: "settings.language_auto" },
  { value: "en", labelKey: "settings.language_en" },
  { value: "fr", labelKey: "settings.language_fr" },
];

export function SettingsPage() {
  const { t } = useTranslation();
  const {
    currentChoice,
    setLanguage,
    autoRecordPrice,
    toggleAutoRecordPrice,
    autoFetch,
    toggleAutoFetch,
  } = useSettings();

  useEffect(() => {
    logger.info("[SettingsPage] mounted");
  }, []);

  return (
    <div className="flex flex-col gap-6 p-6 max-w-md">
      <h2 className="text-2xl font-medium text-m3-on-surface">{t("settings.title")}</h2>

      <section className="flex flex-col gap-3">
        <span className="text-sm font-medium text-m3-on-surface-variant">
          {t("settings.language_label")}
        </span>
        <div className="flex flex-col gap-2">
          {LANGUAGE_OPTIONS.map(({ value, labelKey }) => (
            <label key={value} className="flex items-center gap-3 cursor-pointer group">
              <input
                type="radio"
                name="language"
                value={value}
                checked={currentChoice === value}
                onChange={() => setLanguage(value)}
                className="accent-m3-primary w-4 h-4"
              />
              <span className="text-m3-on-surface group-hover:text-m3-primary transition-colors">
                {t(labelKey)}
              </span>
            </label>
          ))}
        </div>
      </section>

      <section className="flex flex-col gap-2">
        <label className="flex items-start gap-3 cursor-pointer group">
          <input
            id="settings-auto-fetch"
            type="checkbox"
            checked={autoFetch}
            onChange={toggleAutoFetch}
            className="accent-m3-primary w-4 h-4 mt-1"
          />
          <span className="flex flex-col gap-1">
            <span className="text-sm font-medium text-m3-on-surface group-hover:text-m3-primary transition-colors">
              {t("settings.auto_fetch_label")}
            </span>
            <span className="text-xs text-m3-on-surface-variant">
              {t("settings.auto_fetch_description")}
            </span>
          </span>
        </label>
      </section>

      <ScheduledFetchSection />

      <SyncSection />

      <section className="flex flex-col gap-2">
        <label className="flex items-start gap-3 cursor-pointer group">
          <input
            id="settings-auto-record-price"
            type="checkbox"
            checked={autoRecordPrice}
            onChange={toggleAutoRecordPrice}
            className="accent-m3-primary w-4 h-4 mt-1"
          />
          <span className="flex flex-col gap-1">
            <span className="text-sm font-medium text-m3-on-surface group-hover:text-m3-primary transition-colors">
              {t("settings.auto_record_price_label")}
            </span>
            <span className="text-xs text-m3-on-surface-variant">
              {t("settings.auto_record_price_description")}
            </span>
          </span>
        </label>
      </section>
    </div>
  );
}

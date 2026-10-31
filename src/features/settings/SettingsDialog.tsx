import { X } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { selectHasExternalProvider, useAppStore } from "@/lib/store";
import { IconButton } from "@/ui/components/button/IconButton";
import { CheckboxField } from "@/ui/components/field/CheckboxField";
import { SelectField } from "@/ui/components/field/SelectField";
import { ModalContainer } from "@/ui/components/modal/ModalContainer";
import { AgentsAllowedSetting } from "./agent_connection/AgentsAllowedSetting";
import { ReferenceCurrencySection } from "./reference_currency/ReferenceCurrencySection";
import { ScheduledFetchSection } from "./scheduled_fetch/ScheduledFetchSection";
import { SyncSection } from "./sync/SyncSection";
import { type LanguageChoice, useSettings } from "./useSettings";

type SettingsSection = "general" | "prices" | "sync" | "agents";

const LANGUAGE_OPTIONS: { value: LanguageChoice; labelKey: string }[] = [
  { value: "auto", labelKey: "settings.language_auto" },
  { value: "en", labelKey: "settings.language_en" },
  { value: "fr", labelKey: "settings.language_fr" },
];

function GeneralSection() {
  const { t } = useTranslation();
  const { currentChoice, setLanguage, autoRecordPrice, toggleAutoRecordPrice } = useSettings();

  return (
    <>
      <SelectField
        id="settings-language"
        label={t("settings.language_label")}
        value={currentChoice}
        onChange={(event) => setLanguage(event.target.value as LanguageChoice)}
        options={LANGUAGE_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
      />
      <ReferenceCurrencySection />
      <CheckboxField
        id="settings-auto-record-price"
        label={t("settings.auto_record_price_label")}
        description={t("settings.auto_record_price_description")}
        checked={autoRecordPrice}
        onChange={toggleAutoRecordPrice}
      />
    </>
  );
}

function PricesSection() {
  const { t } = useTranslation();
  const { autoFetch, toggleAutoFetch } = useSettings();

  return (
    <>
      <CheckboxField
        id="settings-auto-fetch"
        label={t("settings.auto_fetch_label")}
        description={t("settings.auto_fetch_description")}
        checked={autoFetch}
        onChange={toggleAutoFetch}
      />
      <ScheduledFetchSection />
    </>
  );
}

/** Mounted only while the dialog is open: it opens on General, its settings read afresh. */
function SettingsContent({ titleId, onClose }: { titleId: string; onClose: () => void }) {
  const { t } = useTranslation();
  const [section, setSection] = useState<SettingsSection>("general");
  // MKT-212, SPF-071 — without an External provider, Prices has nothing to show.
  const hasExternalProvider = useAppStore(selectHasExternalProvider);
  const sections: SettingsSection[] = [
    "general",
    ...(hasExternalProvider ? (["prices"] as const) : []),
    "sync",
    "agents",
  ];

  useEffect(() => {
    logger.info("[SettingsDialog] opened");
  }, []);

  return (
    <div className="flex h-[600px] max-h-[90vh] flex-col">
      <header className="flex items-center justify-between border-b border-m3-outline/20 py-4 pl-6 pr-4">
        <h2 id={titleId} className="text-xl font-medium text-m3-on-surface">
          {t("settings.title")}
        </h2>
        <IconButton
          id="settings-close"
          shape="round"
          onClick={onClose}
          aria-label={t("action.close")}
          icon={<X size={20} />}
        />
      </header>
      <div className="flex min-h-0 flex-1">
        <nav
          aria-label={t("settings.sections_label")}
          className="flex w-48 shrink-0 flex-col gap-1 border-r border-m3-outline/20 p-3"
        >
          {sections.map((name) => (
            <button
              type="button"
              key={name}
              id={`settings-section-${name}`}
              aria-current={section === name ? "true" : undefined}
              onClick={() => setSection(name)}
              className={`w-full rounded-xl px-3 py-2 text-left text-sm transition-colors ${
                section === name
                  ? "bg-m3-primary text-m3-on-primary"
                  : "text-m3-on-surface-variant hover:bg-m3-on-surface/5"
              }`}
            >
              {t(`settings.section_${name}`)}
            </button>
          ))}
        </nav>
        <div className="flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-6">
          {section === "general" && <GeneralSection />}
          {section === "prices" && <PricesSection />}
          {section === "sync" && <SyncSection />}
          {section === "agents" && <AgentsAllowedSetting />}
        </div>
      </div>
    </div>
  );
}

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

/** The settings, in named sections, over the screen the user is on. */
export function SettingsDialog({ isOpen, onClose }: SettingsDialogProps) {
  const titleId = useId();

  return (
    <ModalContainer
      id="settings-dialog"
      isOpen={isOpen}
      onClose={onClose}
      maxWidth="max-w-4xl"
      titleId={titleId}
    >
      <SettingsContent titleId={titleId} onClose={onClose} />
    </ModalContainer>
  );
}
